use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::lines;

pub(crate) struct ListText {
    path: PathBuf,
    lines: Vec<String>,
    changed: bool,
}

impl ListText {
    pub(crate) fn read(path: &Path) -> Result<Option<ListText>> {
        let read = lines::read(path).map_err(|source| Error::Associations {
            list: path.to_path_buf(),
            source,
        })?;
        Ok(read.map(|lines| ListText {
            path: path.to_path_buf(),
            lines,
            changed: false,
        }))
    }

    pub(crate) fn fresh(path: &Path) -> ListText {
        ListText {
            path: path.to_path_buf(),
            lines: Vec::new(),
            changed: false,
        }
    }

    pub(crate) fn ids(&self, group: &str, key: &str) -> Vec<String> {
        self.key_lines(group, key)
            .last()
            .and_then(|at| self.lines[*at].split_once('='))
            .map(|(_, value)| super::split_ids(value))
            .unwrap_or_default()
    }

    pub(crate) fn put(&mut self, group: &str, key: &str, ids: &[String]) {
        let key_lines = self.key_lines(group, key);
        let replacement = (!ids.is_empty()).then(|| format!("{key}={};", ids.join(";")));
        match (replacement, key_lines.split_last()) {
            (None, _) => self.remove(&key_lines),
            (Some(line), Some((last, earlier))) => {
                if self.lines[*last] != line {
                    self.lines[*last] = line;
                    self.changed = true;
                }
                self.remove(earlier);
            }
            (Some(line), None) => self.insert(group, line),
        }
    }

    pub(crate) fn write(self) -> Result<()> {
        if !self.changed {
            return Ok(());
        }
        lines::write(&self.path, &self.lines).map_err(|source| match lines::is_read_only(&source) {
            true => Error::AssociationsReadOnly { list: self.path },
            false => Error::Associations {
                list: self.path,
                source,
            },
        })
    }

    fn key_lines(&self, group: &str, key: &str) -> Vec<usize> {
        let mut inside = false;
        let mut key_lines = Vec::new();
        for (at, line) in self.lines.iter().enumerate() {
            if let Some(name) = header(line) {
                inside = name == group;
                continue;
            }
            if inside && key_of(line) == Some(key) {
                key_lines.push(at);
            }
        }
        key_lines
    }

    fn remove(&mut self, key_lines: &[usize]) {
        for at in key_lines.iter().rev() {
            self.lines.remove(*at);
            self.changed = true;
        }
    }

    fn insert(&mut self, group: &str, line: String) {
        self.changed = true;
        let Some(header_index) = self
            .lines
            .iter()
            .rposition(|line| header(line) == Some(group))
        else {
            if self
                .lines
                .last()
                .is_some_and(|last| !last.trim().is_empty())
            {
                self.lines.push(String::new());
            }
            self.lines.push(format!("[{group}]"));
            self.lines.push(line);
            return;
        };
        let closed = self.lines[header_index + 1..]
            .iter()
            .position(|line| header(line).is_some())
            .map_or(self.lines.len(), |after| header_index + 1 + after);
        let mut at = closed;
        while at > header_index + 1 && self.lines[at - 1].trim().is_empty() {
            at -= 1;
        }
        self.lines.insert(at, line);
    }
}

fn header(line: &str) -> Option<&str> {
    line.trim().strip_prefix('[')?.strip_suffix(']')
}

fn key_of(line: &str) -> Option<&str> {
    let line = line.trim_start();
    if line.starts_with('#') {
        return None;
    }
    line.split_once('=').map(|(key, _)| key.trim_end())
}
