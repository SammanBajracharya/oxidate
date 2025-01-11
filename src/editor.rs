use crossterm::{ExecutableCommand, QueueableCommand};
use crossterm::cursor;
use crossterm::style::{self, Color, Stylize};
use crossterm::event::{self, KeyCode, KeyModifiers};
use crossterm::terminal::{self, disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use std::io::{self, Write};

use crate::buffer::Buffer;

enum Action {
    Quit,

    MoveUp,
    MoveDown,
    MoveRight,
    MoveLeft,

    MoveWordForward,
    MoveWordBackward,
    MoveWordEnd,

    MoveToTop,
    MoveToBottom,

    OpenLineAbove,
    OpenLineBelow,

    InsertCharAtCursorPos(char),
    DeleteChar,
    DeleteCharAtCursorPos,
    DeleteCurrentLine,
    NewLine,

    EnterMode(Mode),
    SetWaitingCmd(char),
}

#[derive(Debug, PartialEq)]
enum Mode {
    Normal,
    Insert,
    Visual,
    //Command,
}

pub struct Editor {
    stdout: std::io::Stdout,
    buffer: Buffer,
    cx: usize,
    cy: usize,
    mode: Mode,
    size: (u16, u16),
    vtop: u16,
    vleft: u16,
    waiting_cmd: Option<char>,
}

impl Editor {
    pub fn new(buffer: Buffer) -> io::Result<Self> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        stdout
            .execute(EnterAlternateScreen)?
            .execute(terminal::Clear(terminal::ClearType::All))?;

        Ok(Editor {
            stdout: io::stdout(),
            buffer,
            cx: 0,
            cy: 0,
            size: terminal::size()?,
            mode: Mode::Normal,
            vtop: 0,
            vleft: 0,
            waiting_cmd: None,
        })
    }

    fn vwidth(&self) -> u16 {
        self.size.0 - self.line_number_width() - 1
    }

    fn vheight(&self) -> u16 {
        self.size.1 - 2
    }

    fn line_number_width(&self) -> u16 {
        let total_lines = self.buffer.len() - 1;
        total_lines.to_string().len() as u16
    }

    fn line_length(&self) -> u16 {
        if let Some(line) = self.viewport_line(self.cy as u16) {
            return line.len() as u16;
        }
        0
    }

    fn buffer_line(&self) -> u16 {
        self.vtop + self.cy as u16
    }

    fn viewport_line(&self, n: u16) -> Option<String> {
        let buffer_line = self.vtop + n;
        self.buffer.get(buffer_line as usize)
    }

    pub fn draw(&mut self) -> io::Result<()> {
        self.draw_viewport()?;
        self.draw_statusline()?;
        self.draw_line_numbers()?;

        let x_offset = self.line_number_width() + 2;

        self.stdout.queue(cursor::MoveTo(self.cx as u16 + x_offset, self.cy as u16))?;
        self.stdout.flush()?;

        Ok(())
    }

    pub fn draw_viewport(&mut self) -> io::Result<()> {
        let vwidth = self.vwidth() as usize;
        let start_point = self.line_number_width() + 2;
        for i in 0..self.vheight() {
            let line = self.viewport_line(i).unwrap_or_default();

            self.stdout
                .queue(cursor::MoveTo(start_point, i))?
                .queue(style::Print(format!("{line:<width$}", width = vwidth)))?;
        }
        Ok(())
    }

    pub fn draw_line_numbers(&mut self) -> io::Result<()> {
        let line_number_width = self.line_number_width();
        let editor_border_y = self.vheight().min(self.buffer.len() as u16);
        for line_number in 0..self.vheight() {
            let current_line = if line_number >= editor_border_y {
                format!(" {:>width$} ", "", width = line_number_width as usize)
            } else {
                format!(" {:>width$} ", line_number, width = line_number_width as usize)
            };

            self.stdout.queue(cursor::MoveTo(0, line_number))?;
            self.stdout.queue(style::PrintStyledContent(
                current_line.with(Color::Rgb { r: 128, g: 128, b: 128 })
                    .bold(),
            ))?;
        }
        Ok(())
    }

    pub fn draw_statusline(&mut self) -> io::Result<()> {
        let mode = format!(" {:?} ", self.mode).to_uppercase();
        let file = " src/main.rs";
        let pos = format!(" {}:{} ", self.cx, self.cy);

        let file_width = self.size.0 - mode.len() as u16 - pos.len() as u16 - 2;

        self.stdout.queue(cursor::MoveTo(0, self.size.1 - 2))?;
        self.stdout.queue(style::PrintStyledContent(
            mode.with(Color::Rgb { r: 0, g: 0, b: 0 })
                .bold()
                .on(Color::Rgb { r: 184, g: 144, b: 243 }),
        ))?;
        self.stdout.queue(style::PrintStyledContent(
            ""
                .with(Color::Rgb { r: 184, g: 144, b: 243 })
                .on(Color::Rgb { r: 67, g: 70, b: 89 }),
        ))?;
        self.stdout.queue(style::PrintStyledContent(
            format!("{:<width$}", file, width = file_width as usize)
                .with(Color::Rgb { r: 255, g: 255, b: 255 })
                .bold()
                .on(Color::Rgb { r: 67, g: 70, b: 89 }),
        ))?;
        self.stdout.queue(style::PrintStyledContent(
            ""
                .with(Color::Rgb { r: 184, g: 144, b: 243 })
                .on(Color::Rgb { r: 67, g: 70, b: 89 }),
        ))?;
        self.stdout.queue(style::PrintStyledContent(
            pos.with(Color::Rgb { r: 0, g: 0, b: 0 })
                .bold()
                .on(Color::Rgb { r: 184, g: 144, b: 243 }),
        ))?;

        Ok(())
    }

    pub fn run(&mut self) -> io::Result<()> {
        loop {
            self.draw()?;
            if let Some(action) = self.handle_event(event::read()?)? {
                match action {
                    Action::Quit => break,
                    Action::MoveUp => {
                        self.cy = self.cy.saturating_sub(1);
                        self.cx = self.cx.min(self.buffer.lines[self.cy].len());
                    }
                    Action::MoveDown => {
                        if self.cy.saturating_add(1) < self.buffer.lines.len(){
                            self.cy += 1;
                            self.cx = self.cx.min(self.buffer.lines[self.cy].len());
                        }
                        if self.cy >= self.vheight() as usize {
                            self.cy = (self.vheight() - 1) as usize;
                        }
                    },
                    Action::MoveLeft => {
                        self.cx = self.cx.saturating_sub(1);
                        if self.cx < self.vleft as usize {
                            self.cx = self.vleft as usize;
                        }
                    },
                    Action::MoveRight => {
                        self.cx += 1;
                        if self.cx >= self.line_length() as usize {
                            self.cx = self.line_length() as usize;
                        }
                        if self.cx >= self.vwidth() as usize {
                            self.cx = (self.vwidth() - 1) as usize;
                        }
                    },
                    Action::MoveWordForward => {
                        // TODO: Needs fixing
                        let line = &mut self.buffer.lines[self.cy];
                        if let Some(pos) = line[self.cx..].find(|c: char| c.is_whitespace()) {
                            self.cx += pos + 1;
                        } else {
                            self.cx = line.len();
                        }
                    },
                    Action::MoveWordBackward => {
                        // TODO: Needs fixing
                        let line = &mut self.buffer.lines[self.cy];
                        if let Some(pos) = line[..self.cx].rfind(|c: char| c.is_whitespace()) {
                            self.cx = pos;
                        } else {
                            self.cx = line.len();
                        }
                    },
                    Action::MoveWordEnd => {
                        self.cx = self.buffer.lines[self.cy].len()-1;
                    },
                    Action::MoveToTop => {
                        self.cx = 0;
                        self.cy = 0;
                    },
                    Action::MoveToBottom => {
                        self.cx = 0;
                        self.cy = self.buffer.lines.len() - 1;
                    },
                    Action::OpenLineAbove => {
                        self.buffer.lines.insert(self.cy, String::new());
                        self.mode = Mode::Insert;
                        self.cx = 0;
                    },
                    Action::OpenLineBelow => {
                        self.buffer.lines.insert(self.cy + 1, String::new());
                        self.mode = Mode::Insert;
                        self.cy += 1;
                        self.cx = 0;
                    },
                    Action::InsertCharAtCursorPos(c) => {
                        self.buffer.insert(self.cx as u16, self.buffer_line(), c);
                        self.cx += 1;
                    },
                    Action::DeleteChar => {
                        if self.cx == 0 && self.cy > 0 {
                            let current_line = self.buffer.lines.remove(self.cy);
                            self.cy -= 1;
                            let prev_line = &mut self.buffer.lines[self.cy];
                            self.cx = prev_line.len();
                            prev_line.push_str(&current_line);
                        } else if let Some(line) = self.buffer.lines.get_mut(self.cy) {
                            if self.cx < line.len() { line.remove(self.cx - 1); }
                            else { line.pop(); }
                            self.cx = self.cx.saturating_sub(1);
                        }
                        self.stdout.queue(cursor::MoveTo(self.cx as u16, self.cy as u16))?;
                    },
                    Action::DeleteCharAtCursorPos => {
                        self.buffer.delete(self.cx as u16, self.buffer_line());
                    }
                    Action::DeleteCurrentLine => {
                        self.buffer.remove_line(self.buffer_line());
                        self.cy = self.cy.saturating_sub(1);
                    },
                    Action::NewLine => {
                        if self.cy >= self.buffer.lines.len() {
                            self.buffer.lines.push(String::new());
                        }
                        let line = self.buffer.lines[self.cy].clone();
                        if self.cx < line.len() {
                            let (left, right) = line.split_at(self.cx);
                            self.buffer.lines[self.cy] = left.to_string();
                            self.buffer.lines.insert(self.cy + 1, right.to_string());
                        } else {
                            self.buffer.lines.insert(self.cy + 1, String::new());
                        }
                        self.cx = 0;
                        self.cy += 1;
                    }
                    Action::EnterMode(new_mode) => {
                        if new_mode == Mode::Normal { self.cx = self.cx.saturating_sub(1); }
                        self.mode = new_mode;
                    },
                    Action::SetWaitingCmd(cmd) => {
                        self.waiting_cmd = Some(cmd);
                    },
                }
            }
        }

        Ok(())
    }

    fn handle_event(&mut self, ev: event::Event) -> io::Result<Option<Action>> {
        if matches!(ev, event::Event::Resize(_, _)) {
            self.size = terminal::size()?;
        }

        match self.mode {
            Mode::Normal => self.handle_normal_mode(ev),
            Mode::Insert => self.handle_insert_mode(ev),
            Mode::Visual => self.handle_visual_mode(ev),
            //Mode::Command => self.handle_command_mode(ev),
        }
    }

    // Normal Mode
    fn handle_normal_mode(&mut self, ev: event::Event) -> io::Result<Option<Action>> {
        if let Some(cmd) = self.waiting_cmd {
            self.waiting_cmd = None;
            return self.handle_waiting_cmd(cmd, ev);
        }

        let action = match ev {
            event::Event::Key(event) => {
                let code = event.code;
                let _modifiers = event.modifiers;

                match code {
                    KeyCode::Char('q') => Some(Action::Quit),
                    KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveUp),
                    KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveDown),
                    KeyCode::Right | KeyCode::Char('l') => Some(Action::MoveRight),
                    KeyCode::Left | KeyCode::Char('h') => Some(Action::MoveLeft),
                    KeyCode::Char('w') => Some(Action::MoveWordForward),
                    KeyCode::Char('b') => Some(Action::MoveWordBackward),
                    KeyCode::Char('$') => Some(Action::MoveWordEnd),
                    KeyCode::Char('G') => Some(Action::MoveToBottom),
                    KeyCode::Char('O') => Some(Action::OpenLineAbove),
                    KeyCode::Char('o') => Some(Action::OpenLineBelow),
                    KeyCode::Char('x') => Some(Action::DeleteCharAtCursorPos),
                    KeyCode::Char('i') => Some(Action::EnterMode(Mode::Insert)),
                    KeyCode::Char('a') => {
                        self.cx += 1;
                        Some(Action::EnterMode(Mode::Insert))
                    },
                    KeyCode::Char('v') => Some(Action::EnterMode(Mode::Visual)),
                    // KeyCode::Char(':') => Some(Action::EnterMode(Mode::Command)),
                    KeyCode::Char('d') => Some(Action::SetWaitingCmd('d')),
                    KeyCode::Char('g') => Some(Action::SetWaitingCmd('g')),
                    _ => None,
                }
            },
            _ => None,
        };

        Ok(action)
    }

    fn handle_visual_mode(&mut self, ev: event::Event) -> io::Result<Option<Action>> {
        let action = match ev {
            event::Event::Key(event) => match (event.code, event.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL) |
                (KeyCode::Esc, _) => Some(Action::EnterMode(Mode::Normal)),
                (KeyCode::Char('h'), _) => Some(Action::MoveLeft),
                (KeyCode::Char('j'), _) => Some(Action::MoveDown),
                (KeyCode::Char('k'), _) => Some(Action::MoveUp),
                (KeyCode::Char('l'), _) => Some(Action::MoveRight),
                _ => None,
            },
            _ => None,
        };

        Ok(action)
    }

    fn handle_insert_mode(&mut self, ev: event::Event) -> io::Result<Option<Action>> {
        let action = match ev {
            event::Event::Key(event) => match (event.code, event.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL) |
                (KeyCode::Esc, _)=> Some(Action::EnterMode(Mode::Normal)),
                (KeyCode::Char(c), _) => Some(Action::InsertCharAtCursorPos(c)),
                (KeyCode::Enter, _) => Some(Action::NewLine),
                (KeyCode::Backspace, _) => Some(Action::DeleteChar),
                _ => None,
            },
            _ => None,
        };

        Ok(action)
    }

    fn handle_waiting_cmd(&mut self, cmd: char, ev: event::Event) -> io::Result<Option<Action>> {
        let action = match cmd {
            'd' => match ev {
                event::Event::Key(event) => match event.code {
                    event::KeyCode::Char('d') => Some(Action::DeleteCurrentLine),
                    _ => None,
                },
                _ => None,
            },
            'g' => match ev {
                event::Event::Key(event) => match event.code {
                    event::KeyCode::Char('g') => Some(Action::MoveToTop),
                    _ => None,
                },
                _ => None,
            }
            _ => None,
        };

        Ok(action)
    }

    // Command Mode
    //fn handle_command_mode(&mut self, key_event: KeyEvent) -> io::Result<bool> {
    //    match (key_event.code, key_event.modifiers) {
    //        (KeyCode::Enter, _) => {
    //            let cmd = self.command_buffer.trim();
    //            match cmd {
    //                "q" => return Ok(true),
    //                "w" => {
    //                    // TODO: IMPLEMENT SAVE
    //                    self.status_message = "File Saved".to_string();
    //                },
    //                "wq" => {
    //                    // TODO: IMPLEMENT SAVE AND QUIT
    //                    return Ok(true);
    //                },
    //                _ => {
    //                    self.status_message = format!("Unknown command: {}", cmd);
    //                }
    //            }
    //            self.mode = Mode::Normal;
    //            self.command_buffer.clear();
    //        },
    //        (KeyCode::Char('c'), KeyModifiers::CONTROL) |
    //        (KeyCode::Esc, _) => {
    //            self.mode = Mode::Normal;
    //            self.command_buffer.clear();
    //            self.status_message = "-- NORMAL --".to_string();
    //        },
    //        (KeyCode::Backspace, _) => {
    //            self.command_buffer.pop();
    //            self.status_message = format!(":{}", self.command_buffer);
    //        },
    //        (KeyCode::Char(c), _) => {
    //            self.command_buffer.push(c);
    //            self.status_message = format!(":{}", self.command_buffer);
    //        },
    //        _ => {}
    //    }
    //    Ok(false)
    //}

    pub fn cleanup(&mut self) -> io::Result<()> {
        self.stdout.execute(LeaveAlternateScreen)?;
        disable_raw_mode()?;

        Ok(())
    }
}


