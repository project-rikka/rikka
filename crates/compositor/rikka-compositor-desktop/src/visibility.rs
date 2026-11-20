use crate::ids::SnapshotId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    Shown,
    OnHiddenWorkspace,
    Minimized,
    Exiting(SnapshotId),
}

impl Visibility {
    pub fn shown(self) -> bool {
        self == Self::Shown
    }
}
