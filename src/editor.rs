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

    AddChar(char),
    DeleteChar,
    NewLine,

    EnterMode(Mode),
}

#[derive(Debug)]
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
            // command_buffer: String::new(),
        })
    }

    fn vwidth(&self) -> u16 {
        self.size.0
    }

    fn vheight(&self) -> u16 {
        self.size.1 - 2
    }

    fn line_length(&self) -> u16 {
        if let Some(line) = self.viewport_line(self.cy as u16) {
            return line.len() as u16;
        }
        0
    }

    fn viewport_line(&self, n: u16) -> Option<String> {
        let buffer_line = self.vtop + n;
        self.buffer.get(buffer_line as usize)
    }

    pub fn draw(&mut self) -> io::Result<()> {
        self.draw_viewport()?;
        self.draw_statusline()?;
        self.stdout.queue(cursor::MoveTo(self.cx as u16, self.cy as u16))?;
        self.stdout.flush()?;

        Ok(())
    }

    pub fn draw_viewport(&mut self) -> io::Result<()> {
        let vwidth = self.vwidth() as usize;
        for i in 0..self.vheight() {
            let line = match self.viewport_line(i) {
                None => String::new(),
                Some(s) => s,
            };

            self.stdout
                .queue(cursor::MoveTo(0, i))?
                .queue(style::Print(format!("{line:<width$}", width = vwidth,)))?;
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
                    Action::MoveUp => self.cy = self.cy.saturating_sub(1),
                    Action::MoveDown => {
                        self.cy += 1;
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
                    Action::EnterMode(new_mode) => self.mode = new_mode,
                    Action::AddChar(c) => {
                        if self.cy >= self.buffer.lines.len() { self.buffer.lines.push(String::new()); }

                        let line = &mut self.buffer.lines[self.cy];

                        if self.cx <= line.len() { line.insert(self.cx, c); }
                        else { line.push(c); }

                        self.cx += 1;
                        self.stdout.queue(cursor::MoveTo(self.cx as u16, self.cy as u16))?;
                        self.stdout.queue(style::Print(c))?;
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
        let action = match ev {
            event::Event::Key(event) => match event.code {
                KeyCode::Char('q') => Some(Action::Quit),
                KeyCode::Char('i') => Some(Action::EnterMode(Mode::Insert)),
                KeyCode::Char('v') => Some(Action::EnterMode(Mode::Visual)),
                // KeyCode::Char(':') => Some(Action::EnterMode(Mode::Command)),
                KeyCode::Left | KeyCode::Char('h') => Some(Action::MoveLeft),
                KeyCode::Down | KeyCode::Char('j') => Some(Action::MoveDown),
                KeyCode::Up | KeyCode::Char('k') => Some(Action::MoveUp),
                KeyCode::Right | KeyCode::Char('l') => Some(Action::MoveRight),
                _ => None,
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
                (KeyCode::Char(c), _) => Some(Action::AddChar(c)),
                (KeyCode::Enter, _) => Some(Action::NewLine),
                (KeyCode::Backspace, _) => Some(Action::DeleteChar),
                _ => None,
            },
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
