use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use cli_command::{Shell, write_completions};

#[derive(Debug, Parser)]
#[command(about = "Write rikka completions to a file")]
struct Generate {
    #[arg(value_enum)]
    shell: Shell,
    into: PathBuf,
}

fn main() -> ExitCode {
    let generate = Generate::parse();
    let mut script = Vec::new();
    write_completions(generate.shell, &mut script);
    match std::fs::write(&generate.into, script) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {}: {error}", generate.into.display());
            ExitCode::FAILURE
        }
    }
}
