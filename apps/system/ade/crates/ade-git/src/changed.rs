use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::change::{Change, State};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changed {
    files: BTreeMap<PathBuf, State>,
    folders: BTreeMap<PathBuf, State>,
}

impl Changed {
    #[must_use]
    pub fn new(root: &Path, changes: &[Change]) -> Changed {
        let mut changed = Changed::default();
        for change in changes {
            let at = root.join(change.path());
            for folder in at
                .ancestors()
                .skip(1)
                .take_while(|folder| folder.starts_with(root))
            {
                let previous = changed.folders.get(folder).copied();
                changed
                    .folders
                    .insert(folder.to_owned(), rolled(previous, change.state()));
            }
            changed.files.insert(at, change.state());
        }
        changed
    }

    #[must_use]
    pub fn file(&self, at: &Path) -> Option<State> {
        self.files.get(at).copied()
    }

    #[must_use]
    pub fn folder(&self, at: &Path) -> Option<State> {
        self.folders.get(at).copied()
    }
}

fn rolled(previous: Option<State>, state: State) -> State {
    let Some(previous) = previous else {
        return state;
    };
    let fresh = |state| matches!(state, State::Added | State::Untracked);
    match (previous, state) {
        (one, two) if fresh(one) && fresh(two) => State::Untracked,
        (State::Deleted, State::Deleted) => State::Deleted,
        _ => State::Modified,
    }
}
