use std::process::{Command, ExitCode};
use std::sync::Arc;

use files_core::Location;
use files_open::Applications;
use files_vfs::Vfs;
use rikka_file_operations::Store;
use rikka_mime::Database;
use rikka_open::spawn_detached;

fn main() -> ExitCode {
    let targets: Vec<String> = std::env::args().skip(1).collect();
    if targets.is_empty() {
        eprintln!("usage: rikka-open <file|uri>...");
        return ExitCode::FAILURE;
    }

    let mime = Arc::new(Database::load());
    let vfs = Vfs::new(Arc::clone(&mime));
    let applications = Applications::load();

    let mut opened = true;
    for target in &targets {
        if let Err(error) = open_one(target, &vfs, &applications, &mime) {
            eprintln!("rikka-open: {target}: {error}");
            opened = false;
        }
    }
    if opened {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn open_one(
    target: &str,
    vfs: &Vfs,
    applications: &Applications,
    mime: &Database,
) -> Result<(), String> {
    let Some(at) = local(target) else {
        return elsewhere(target);
    };
    let entry = vfs.stat(&at).map_err(|error| error.to_string())?;
    applications
        .open(&entry, mime)
        .map_err(|error| error.to_string())
}

fn local(target: &str) -> Option<Location> {
    match Location::parse(target) {
        Some(at) => at.is_local().then_some(at),
        None => Some(Location::from_path(target)),
    }
}

fn elsewhere(target: &str) -> Result<(), String> {
    spawn_detached(Command::new("xdg-open").arg(target))
        .map(drop)
        .map_err(|error| error.to_string())
}
