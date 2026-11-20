use std::fs::{self, File};
use std::io;
use std::path::PathBuf;

use rikka_dirs::Paths;

use super::Journal;
use crate::retention::keep_newest;

const COMPOSITOR_LOG: &str = "compositor.log";
const COMPOSITOR_PREVIOUS: &str = "compositor.log.1";
const SHELL_LOG: &str = "shell.log";
const SHELL_PREVIOUS: &str = "shell.log.1";
const INSTANCES: &str = "compositors";
const KEPT_INSTANCE_LOGS: usize = 16;

pub(crate) fn state_directory() -> io::Result<PathBuf> {
    let paths = Paths::resolve().map_err(|error| io::Error::new(io::ErrorKind::NotFound, error))?;
    Ok(paths.state_dir().to_path_buf())
}

pub(super) fn open_session_log() -> io::Result<(PathBuf, File)> {
    rotated(COMPOSITOR_LOG, COMPOSITOR_PREVIOUS)
}

pub(super) fn open_instance_log(backend: &str, socket: &str) -> io::Result<(PathBuf, File)> {
    instance(format!("{backend}-{}.log", file_safe(socket)))
}

pub(crate) fn open_shell_log(journal: Journal, socket: &str) -> io::Result<(PathBuf, File)> {
    match journal.backend() {
        None => rotated(SHELL_LOG, SHELL_PREVIOUS),
        Some(backend) => instance(format!("{backend}-{}-shell.log", file_safe(socket))),
    }
}

fn rotated(name: &str, previous: &str) -> io::Result<(PathBuf, File)> {
    let directory = state_directory()?;
    fs::create_dir_all(&directory)?;
    let path = directory.join(name);
    match fs::rename(&path, directory.join(previous)) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        Ok(()) | Err(_) => {}
    }
    let file = File::create(&path)?;
    Ok((path, file))
}

fn instance(name: String) -> io::Result<(PathBuf, File)> {
    let directory = state_directory()?.join(INSTANCES);
    fs::create_dir_all(&directory)?;
    let path = directory.join(name);
    let file = File::create(&path)?;
    keep_newest(&directory, KEPT_INSTANCE_LOGS);
    Ok((path, file))
}

fn file_safe(socket: &str) -> String {
    let name = socket.rsplit('/').next().unwrap_or(socket);
    name.chars().map(file_safe_char).collect()
}

fn file_safe_char(c: char) -> char {
    match c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
        true => c,
        false => '_',
    }
}
