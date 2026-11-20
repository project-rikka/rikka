use rikka_sync::{Mutex, MutexGuard};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::writer::{self, Precondition};

const INDEX: &str = "index.jsonl";

#[derive(Clone, Debug)]
pub struct History {
    state: Arc<Mutex<HistoryState>>,
}

#[derive(Debug, Default)]
pub struct RewindReport {
    pub restored: Vec<PathBuf>,
    pub failed: Vec<Error>,
}

#[derive(Debug)]
struct HistoryState {
    folder: PathBuf,
    checkpoint: u64,
    entries: Option<Vec<Entry>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Entry {
    checkpoint: u64,
    path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    before: Option<String>,
}

impl History {
    #[must_use]
    pub fn open(folder: impl Into<PathBuf>) -> History {
        History {
            state: Arc::new(Mutex::new(HistoryState {
                folder: folder.into(),
                checkpoint: 0,
                entries: None,
            })),
        }
    }

    pub fn set_checkpoint(&self, checkpoint: u64) {
        self.state().checkpoint = checkpoint;
    }

    pub fn record(&self, path: &Path, before: Option<&[u8]>) -> Result<()> {
        let mut state = self.state();
        let checkpoint = state.checkpoint;
        let record_error = |reason: String| Error::HistoryWrite {
            path: path.display().to_string(),
            reason,
        };
        let already_recorded = state
            .entries()
            .map_err(|error| record_error(error.to_string()))?
            .iter()
            .any(|entry| entry.checkpoint == checkpoint && entry.path == path);
        if already_recorded {
            return Ok(());
        }
        let before = before
            .map(|bytes| state.store_snapshot(bytes))
            .transpose()
            .map_err(|error| record_error(error.to_string()))?;
        state
            .append(Entry {
                checkpoint,
                path: path.to_owned(),
                before,
            })
            .map_err(|error| record_error(error.to_string()))
    }

    pub fn changed_since(&self, checkpoint: u64) -> Result<Vec<PathBuf>> {
        let mut state = self.state();
        let mut changed = Vec::new();
        for (path, before) in state.entries_since(checkpoint)? {
            if state.read_snapshot(before.as_deref())? != read_current(&path)? {
                changed.push(path);
            }
        }
        Ok(changed)
    }

    pub fn rewind(&self, checkpoint: u64) -> Result<RewindReport> {
        let mut state = self.state();
        let mut report = RewindReport::default();
        let mut unrestored = HashSet::new();
        for (path, before) in state.entries_since(checkpoint)? {
            match state.restore(&path, before.as_deref()) {
                Ok(true) => report.restored.push(path),
                Ok(false) => {}
                Err(error) => {
                    report.failed.push(error);
                    unrestored.insert(path);
                }
            }
        }
        state.prune(checkpoint, &unrestored)?;
        Ok(report)
    }

    fn state(&self) -> MutexGuard<'_, HistoryState> {
        self.state.lock()
    }
}

impl HistoryState {
    fn entries(&mut self) -> Result<&mut Vec<Entry>> {
        if self.entries.is_none() {
            self.entries = Some(read_index(&self.folder.join(INDEX))?);
        }
        Ok(self.entries.get_or_insert_default())
    }

    fn entries_since(&mut self, since: u64) -> Result<Vec<(PathBuf, Option<String>)>> {
        let mut seen = HashSet::new();
        Ok(self
            .entries()?
            .iter()
            .filter(|entry| entry.checkpoint >= since && seen.insert(entry.path.clone()))
            .map(|entry| (entry.path.clone(), entry.before.clone()))
            .collect())
    }

    fn read_snapshot(&self, before: Option<&str>) -> Result<Option<Vec<u8>>> {
        let Some(name) = before else {
            return Ok(None);
        };
        let path = self.folder.join(name);
        std::fs::read(&path)
            .map(Some)
            .map_err(|error| io(&path, &error))
    }

    fn restore(&self, path: &Path, before: Option<&str>) -> Result<bool> {
        let snapshot = self.read_snapshot(before)?;
        if snapshot == read_current(path)? {
            return Ok(false);
        }
        match snapshot {
            Some(bytes) => writer::write(path, &bytes, &Precondition::Any)?,
            None => remove_if_exists(path)?,
        }
        Ok(true)
    }

    fn store_snapshot(&self, bytes: &[u8]) -> Result<String> {
        let name = sha256_hex(bytes);
        let path = self.folder.join(&name);
        if !path.exists() {
            writer::write(&path, bytes, &Precondition::Any)?;
        }
        Ok(name)
    }

    fn append(&mut self, entry: Entry) -> Result<()> {
        let index = self.folder.join(INDEX);
        let mut line = serde_json::to_vec(&entry).map_err(|error| Error::Io {
            path: index.display().to_string(),
            reason: error.to_string(),
        })?;
        line.push(b'\n');
        std::fs::create_dir_all(&self.folder).map_err(|error| io(&self.folder, &error))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&index)
            .map_err(|error| io(&index, &error))?;
        file.write_all(&line)
            .and_then(|()| file.sync_data())
            .map_err(|error| io(&index, &error))?;
        self.entries()?.push(entry);
        Ok(())
    }

    fn prune(&mut self, since: u64, unrestored: &HashSet<PathBuf>) -> Result<()> {
        let folder = self.folder.clone();
        let entries = self.entries()?;
        let (pruned, retained): (Vec<Entry>, Vec<Entry>) = std::mem::take(entries)
            .into_iter()
            .partition(|entry| entry.checkpoint >= since && !unrestored.contains(&entry.path));
        *entries = retained;
        if pruned.is_empty() {
            return Ok(());
        }

        let mut index = Vec::new();
        for entry in entries.iter() {
            serde_json::to_writer(&mut index, entry).map_err(|error| Error::Io {
                path: folder.join(INDEX).display().to_string(),
                reason: error.to_string(),
            })?;
            index.push(b'\n');
        }
        writer::write(&folder.join(INDEX), &index, &Precondition::Any)?;

        let referenced: HashSet<&str> = entries
            .iter()
            .filter_map(|entry| entry.before.as_deref())
            .collect();
        for name in pruned.iter().filter_map(|entry| entry.before.as_deref()) {
            if !referenced.contains(name) {
                remove_if_exists(&folder.join(name))?;
            }
        }
        Ok(())
    }
}

fn read_index(path: &Path) -> Result<Vec<Entry>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io(path, &error)),
    };
    let lines = BufReader::new(file)
        .split(b'\n')
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|error| io(path, &error))?;
    let last = lines.len().saturating_sub(1);
    let mut entries = Vec::with_capacity(lines.len());
    for (index, line) in lines.iter().enumerate() {
        match serde_json::from_slice::<Entry>(line) {
            Ok(entry) => entries.push(entry),
            Err(_) if index == last => {}
            Err(error) => {
                return Err(Error::Io {
                    path: path.display().to_string(),
                    reason: format!("line {}: {error}", index + 1),
                });
            }
        }
    }
    Ok(entries)
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(io(path, &error)),
        _ => Ok(()),
    }
}

fn read_current(path: &Path) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io(path, &error)),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    rikka_types::hex::encode(Sha256::digest(bytes))
}

fn io(path: &Path, error: &std::io::Error) -> Error {
    Error::Io {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}
