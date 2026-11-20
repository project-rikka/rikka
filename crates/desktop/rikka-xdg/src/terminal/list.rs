use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::lines;

const COMMENT: char = '#';
const DIRECTIVE: char = '/';
const EXCLUDE: char = '-';
const PROTECT: char = '+';

pub(super) struct TerminalList {
    path: PathBuf,
    lines: Vec<String>,
    changed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Line<'a> {
    Chosen {
        id: &'a str,
        action: Option<&'a str>,
    },
    Excluded(&'a str),
    Protected(&'a str),
}

impl<'a> Line<'a> {
    fn parse(line: &'a str) -> Option<Line<'a>> {
        let line = line.trim();
        if line.is_empty() || line.starts_with(COMMENT) || line.starts_with(DIRECTIVE) {
            return None;
        }
        if let Some(id) = line.strip_prefix(EXCLUDE) {
            return Some(Line::Excluded(id.trim()));
        }
        if let Some(id) = line.strip_prefix(PROTECT) {
            return Some(Line::Protected(id.trim()));
        }
        Some(match line.split_once(':') {
            Some((id, action)) => Line::Chosen {
                id,
                action: Some(action).filter(|action| !action.is_empty()),
            },
            None => Line::Chosen {
                id: line,
                action: None,
            },
        })
    }

    pub(super) fn id(&self) -> &'a str {
        match self {
            Line::Chosen { id, .. } | Line::Excluded(id) | Line::Protected(id) => id,
        }
    }
}

impl TerminalList {
    pub(super) fn read(path: &Path) -> Result<Option<TerminalList>> {
        let read = lines::read(path).map_err(|source| Error::Terminals {
            list: path.to_path_buf(),
            source,
        })?;
        Ok(read.map(|lines| TerminalList {
            path: path.to_path_buf(),
            lines,
            changed: false,
        }))
    }

    pub(super) fn fresh(path: &Path) -> TerminalList {
        TerminalList {
            path: path.to_path_buf(),
            lines: Vec::new(),
            changed: false,
        }
    }

    pub(super) fn lines(&self) -> impl Iterator<Item = Line<'_>> {
        self.lines.iter().filter_map(|line| Line::parse(line))
    }

    pub(super) fn chooses_any(&self) -> bool {
        self.lines().any(|line| matches!(line, Line::Chosen { .. }))
    }

    pub(super) fn choose_first(&mut self, id: &str) {
        let mut lines = self.lines.clone();
        lines.retain(|line| !matches!(Line::parse(line), Some(line) if line.id() == id));
        let at = lines
            .iter()
            .position(|line| matches!(Line::parse(line), Some(Line::Chosen { .. })))
            .unwrap_or(lines.len());
        lines.insert(at, id.to_owned());
        if lines != self.lines {
            self.lines = lines;
            self.changed = true;
        }
    }

    pub(super) fn drop_chosen(&mut self) {
        let before = self.lines.len();
        self.lines
            .retain(|line| !matches!(Line::parse(line), Some(Line::Chosen { .. })));
        self.changed |= self.lines.len() != before;
    }

    pub(super) fn write(self) -> Result<()> {
        if !self.changed {
            return Ok(());
        }
        lines::write(&self.path, &self.lines).map_err(|source| match lines::is_read_only(&source) {
            true => Error::TerminalsReadOnly { list: self.path },
            false => Error::Terminals {
                list: self.path,
                source,
            },
        })
    }
}
