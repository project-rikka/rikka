mod desktop;
mod remote;
mod seam;
mod settings;
mod wallpaper;
mod window;
mod workspace;

use std::io::{self, Write};

use cli_command::Command;

use crate::error::Result;
use crate::printer::Printer;

pub(crate) fn run(command: Command, printer: Printer) -> Result<()> {
    match command {
        Command::Settings(command) => settings::run(command, printer),
        Command::Window(command) => window::run(command, printer),
        Command::Workspace(command) => workspace::run(command, printer),
        Command::Wallpaper(command) => wallpaper::run(command, printer),
        Command::Desktop(command) => desktop::run(command, printer),
        Command::Remote(command) => remote::run(command, printer),
        Command::Seam(command) => seam::run(command, printer),
        Command::Completions { shell } => {
            let mut script = Vec::new();
            cli_command::write_completions(shell, &mut script);
            io::stdout().lock().write_all(&script)?;
            Ok(())
        }
    }
}
