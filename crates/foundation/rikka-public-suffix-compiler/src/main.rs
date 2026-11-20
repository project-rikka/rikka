mod emit;
mod error;
mod list;
mod punycode;

use std::path::PathBuf;
use std::process::ExitCode;

use error::{Error, Result};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rikka-public-suffix-compiler: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut arguments = std::env::args_os().skip(1).map(PathBuf::from);
    let (Some(input), Some(output), None) = (arguments.next(), arguments.next(), arguments.next())
    else {
        return Err(Error::Usage);
    };
    let text = std::fs::read_to_string(&input).map_err(|source| Error::Read {
        path: input.clone(),
        source,
    })?;
    let rules = list::parse(&text)?;
    std::fs::write(&output, emit::table(&rules)).map_err(|source| Error::Write {
        path: output,
        source,
    })
}
