use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use flate2::read::GzDecoder;

use crate::error::{Error, Result};
use crate::folder::entries;

const SCREENSHOTS: &str = "screenshots";
const LOGS: &str = "logs";
const CRASH_REPORTS: &str = "crash-reports";
const MAX_LOG: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screenshot {
    pub path: PathBuf,
    pub name: String,
    pub taken: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LogKind {
    Game,
    Crash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogFile {
    pub path: PathBuf,
    pub name: String,
    pub kind: LogKind,
    pub modified: i64,
}

fn modified(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

fn files(folder: &Path, keep: impl Fn(&str) -> bool) -> Result<Vec<(PathBuf, String)>> {
    let mut found = Vec::new();
    for entry in entries(folder)? {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if keep(name) && path.is_file() {
            found.push((path.clone(), name.to_owned()));
        }
    }
    Ok(found)
}

pub(crate) fn screenshots(root: &Path) -> Result<Vec<Screenshot>> {
    let mut shots: Vec<_> = files(&root.join(SCREENSHOTS), |name| {
        name.to_ascii_lowercase().ends_with(".png")
    })?
    .into_iter()
    .map(|(path, name)| Screenshot {
        taken: modified(&path),
        path,
        name,
    })
    .collect();
    shots.sort_by_key(|shot| std::cmp::Reverse(shot.taken));
    Ok(shots)
}

pub(crate) fn logs(root: &Path) -> Result<Vec<LogFile>> {
    let game = files(&root.join(LOGS), |name| {
        name.ends_with(".log") || name.ends_with(".log.gz")
    })?
    .into_iter()
    .map(|(path, name)| (path, name, LogKind::Game));
    let crashes = files(&root.join(CRASH_REPORTS), |name| name.ends_with(".txt"))?
        .into_iter()
        .map(|(path, name)| (path, name, LogKind::Crash));
    let mut logs: Vec<_> = game
        .chain(crashes)
        .map(|(path, name, kind)| LogFile {
            modified: modified(&path),
            path,
            name,
            kind,
        })
        .collect();
    logs.sort_by_key(|log| std::cmp::Reverse(log.modified));
    Ok(logs)
}

pub fn read_log(path: &Path) -> Result<String> {
    let file = fs::File::open(path).map_err(Error::io(path))?;
    let mut bytes = Vec::new();
    let compressed = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gz"));
    match compressed {
        true => GzDecoder::new(file).take(MAX_LOG).read_to_end(&mut bytes),
        false => file.take(MAX_LOG).read_to_end(&mut bytes),
    }
    .map_err(Error::io(path))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
