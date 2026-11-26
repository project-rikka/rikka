use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use rikka_settings::Catalog;

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let Some(output) = arguments.next().map(PathBuf::from) else {
        eprintln!("usage: rikka-settings-catalog <output> <printer>…");
        return ExitCode::FAILURE;
    };
    match merged(arguments).and_then(|catalog| {
        std::fs::write(&output, catalog.to_json())
            .map_err(|error| format!("failed to write {}: {error}", output.display()))
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn merged(printers: impl Iterator<Item = OsString>) -> Result<Catalog, String> {
    let parts = printers.map(printed).collect::<Result<Vec<_>, _>>()?;
    Catalog::merge(parts).map_err(|error| error.to_string())
}

fn printed(printer: OsString) -> Result<Catalog, String> {
    let shown = PathBuf::from(&printer).display().to_string();
    let output = Command::new(&printer)
        .output()
        .map_err(|error| format!("failed to run {shown}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{shown} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|error| format!("{shown} printed something other than UTF-8: {error}"))?;
    Catalog::from_json(&text).map_err(|error| format!("{shown}: {error}"))
}
