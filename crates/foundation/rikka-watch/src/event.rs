use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Changed(Change),
    Lost,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub path: PathBuf,
    pub kind: ChangeKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChangeKind {
    Created,
    Removed,
    Modified,
    Written,
    Metadata,
    Opened,
    Closed,
}

impl ChangeKind {
    #[must_use]
    pub fn alters_listing(self) -> bool {
        matches!(self, ChangeKind::Created | ChangeKind::Removed)
    }
}
