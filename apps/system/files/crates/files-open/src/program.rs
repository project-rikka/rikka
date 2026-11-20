use std::path::Path;
use std::process::Command;

use files_core::Entry;
use rikka_open::{Result, spawn_detached};

const RUNNABLE: &[&str] = &[
    "application/x-executable",
    "application/x-pie-executable",
    "application/x-sharedlib",
    "application/x-shellscript",
    "application/vnd.appimage",
];

#[must_use]
pub fn is_program(entry: &Entry) -> bool {
    entry.is_executable() && RUNNABLE.contains(&entry.mime.as_str())
}

pub fn run(program: &Path) -> Result<()> {
    let mut command = Command::new(program);
    if let Some(directory) = program.parent() {
        command.current_dir(directory);
    }
    spawn_detached(&mut command).map(drop)
}
