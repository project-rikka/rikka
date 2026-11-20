mod commands;
mod error;
mod printer;
mod session;
mod session_settings;
mod table;

use std::error::Error as _;
use std::process::ExitCode;

use clap::Parser;
use cli_command::Cli;

use crate::printer::{Format, Printer};

fn main() -> ExitCode {
    rikka_log::install("warn");
    let cli = Cli::parse();
    let printer = Printer::new(match cli.json {
        true => Format::Json,
        false => Format::Text,
    });
    match commands::run(cli.command, printer) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.is_closed_output() => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            let mut cause = error.source();
            while let Some(source) = cause {
                eprintln!("  caused by: {source}");
                cause = source.source();
            }
            ExitCode::FAILURE
        }
    }
}
