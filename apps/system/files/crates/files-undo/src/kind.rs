use rikka_file_operations::TaskKind;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Copy,
    Move,
    Rename,
    Trash,
    Restore,
    NewFolder,
    NewFile,
}

impl Kind {
    #[must_use]
    pub fn from_task(kind: TaskKind) -> Option<Self> {
        match kind {
            TaskKind::Copy => Some(Self::Copy),
            TaskKind::Move => Some(Self::Move),
            TaskKind::Trash => Some(Self::Trash),
            TaskKind::Restore => Some(Self::Restore),
            TaskKind::Delete | TaskKind::Compress | TaskKind::Extract | TaskKind::Convert => None,
        }
    }
}
