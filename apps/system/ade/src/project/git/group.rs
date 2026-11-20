use ade_git::{Change, State};
use rikka_app::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Group {
    New,
    Modified,
    Deleted,
    Moved,
    Other,
}

impl Group {
    pub(super) fn from_change(change: &Change) -> Group {
        match change.state() {
            State::Added | State::Untracked => Group::New,
            State::Modified => Group::Modified,
            State::Deleted => Group::Deleted,
            State::Renamed => Group::Moved,
            State::Copied | State::Retyped | State::Conflicted => Group::Other,
        }
    }

    pub(super) fn label(self) -> Label {
        match self {
            Group::New => tr!("ade.git.group.new"),
            Group::Modified => tr!("ade.git.group.modified"),
            Group::Deleted => tr!("ade.git.group.deleted"),
            Group::Moved => tr!("ade.git.group.moved"),
            Group::Other => tr!("ade.git.group.other"),
        }
    }
}

pub(super) fn grouped(changes: &[Change]) -> Vec<&Change> {
    let mut grouped: Vec<&Change> = changes.iter().collect();
    grouped.sort_by_key(|change| Group::from_change(change));
    grouped
}

pub(super) fn foremost(changes: &[Change]) -> Option<&Change> {
    changes
        .iter()
        .min_by_key(|change| Group::from_change(change))
}
