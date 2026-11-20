use std::path::PathBuf;
use std::process::ExitCode;

use rikka_symbols_scanner::{Error, Kind, Result};

enum Mode {
    Table(Kind),
    Symbolic(String),
}

struct Task {
    job: Mode,
    listing: PathBuf,
    font: PathBuf,
    output: PathBuf,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rikka-symbols-scanner: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let task = task(std::env::args().skip(1))?;
    let output_text = match task.job {
        Mode::Table(kind) => rikka_symbols_scanner::constants(kind, &task.listing, &task.font)?,
        Mode::Symbolic(name) => {
            rikka_symbols_scanner::symbolic_icon(&task.listing, &task.font, &name)?
        }
    };
    std::fs::write(&task.output, output_text).map_err(|source| Error::Write {
        path: task.output,
        source,
    })
}

fn task(args: impl Iterator<Item = String>) -> Result<Task> {
    let args: Vec<String> = args.collect();
    let (job, listing, font, output) = match args.as_slice() {
        [mode, listing, font, name, output] if mode == "symbolic" => {
            (Mode::Symbolic(name.clone()), listing, font, output)
        }
        [mode, rest @ ..] if mode == "symbolic" => {
            return Err(Error::Usage(format!(
                "symbolic expects 4 arguments, got {}",
                rest.len()
            )));
        }
        [kind, listing, font, output] => {
            let kind =
                Kind::named(kind).ok_or_else(|| Error::Usage(format!("unknown kind {kind:?}")))?;
            (Mode::Table(kind), listing, font, output)
        }
        _ => {
            return Err(Error::Usage(format!(
                "expected 4 arguments, got {}",
                args.len()
            )));
        }
    };
    Ok(Task {
        job,
        listing: listing.into(),
        font: font.into(),
        output: output.into(),
    })
}
