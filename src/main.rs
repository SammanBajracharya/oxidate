use crossterm::{terminal, ExecutableCommand};
use editor::Editor;
use buffer::Buffer;
use std::{io::stdout, io, panic};

mod editor;
mod buffer;

fn main() -> io::Result<()> {
    let file = std::env::args().nth(1);
    let buffer = Buffer::from_file(file);
    let mut editor = Editor::new(buffer)?;

    panic::set_hook(Box::new(|info| {
        _ = stdout().execute(terminal::LeaveAlternateScreen);
        _ = terminal::disable_raw_mode();
        eprintln!("{}", info);
    }));

    editor.run()?;
    editor.cleanup()
}

