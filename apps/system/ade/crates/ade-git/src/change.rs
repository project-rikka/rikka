use std::path::{Path, PathBuf};

use crate::stat::Stat;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Retyped,
    Untracked,
    Conflicted,
}

impl State {
    pub(crate) fn read(staged: char, worktree: char) -> State {
        match (staged, worktree) {
            ('D', _) | (_, 'D') => State::Deleted,
            ('R', _) => State::Renamed,
            ('C', _) => State::Copied,
            ('A', _) => State::Added,
            ('T', _) | (_, 'T') => State::Retyped,
            _ => State::Modified,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    path: PathBuf,
    from: Option<PathBuf>,
    state: State,
    stat: Option<Stat>,
}

impl Change {
    pub(crate) fn new(path: PathBuf, from: Option<PathBuf>, state: State) -> Change {
        Change {
            path,
            from,
            state,
            stat: None,
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    #[must_use]
    pub fn from(&self) -> Option<&Path> {
        self.from.as_deref()
    }

    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    #[must_use]
    pub fn stat(&self) -> Option<Stat> {
        self.stat
    }

    pub(crate) fn count(&mut self, stat: Option<Stat>) {
        self.stat = stat;
    }

    pub fn paths(&self) -> impl Iterator<Item = PathBuf> {
        std::iter::once(self.path.clone()).chain(self.from.clone())
    }
}
