use std::fmt;
use std::path::PathBuf;

use rikka_proton_drive_sync_state::Changes;

use crate::status::ItemState;

pub(crate) enum Invalidation {
    Nothing,
    Local(PathBuf),
    Remote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SkipReason {
    RemoteChanged,
    LinkTargetMissing,
    SyncedItemMissing,
    SyncSideMissing,
    ItemMissing,
    NoParentFolder,
    MovedConcurrently,
    RemoteFolderMissing,
    FolderMissing,
    RemoteFolderUnreadable,
    NoAvailableName,
    RemoteItemMissing,
    ChangedLocally,
    ChangedBeforeUpload,
    ChangedDuringUpload,
    UploadedFileUnreadable,
    NotATransfer,
}

impl fmt::Display for SkipReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            SkipReason::RemoteChanged => "remote item changed",
            SkipReason::LinkTargetMissing => "item to link missing",
            SkipReason::SyncedItemMissing => "synced item missing",
            SkipReason::SyncSideMissing => "local or remote side missing",
            SkipReason::ItemMissing => "item missing",
            SkipReason::NoParentFolder => "item has no parent folder",
            SkipReason::MovedConcurrently => "item moved concurrently",
            SkipReason::RemoteFolderMissing => "remote folder missing",
            SkipReason::FolderMissing => "folder missing",
            SkipReason::RemoteFolderUnreadable => "failed to read new remote folder",
            SkipReason::NoAvailableName => "no available folder name",
            SkipReason::RemoteItemMissing => "remote file or folder missing",
            SkipReason::ChangedLocally => "local item changed",
            SkipReason::ChangedBeforeUpload => "local file changed before upload",
            SkipReason::ChangedDuringUpload => "local file changed during upload",
            SkipReason::UploadedFileUnreadable => "failed to reread uploaded file",
            SkipReason::NotATransfer => "operation is not a transfer",
        })
    }
}

pub(crate) enum Outcome {
    Done(Effect),
    Skipped {
        reason: SkipReason,
        invalidation: Invalidation,
    },
}

impl Outcome {
    pub(crate) fn skipped(reason: SkipReason) -> Outcome {
        Outcome::Skipped {
            reason,
            invalidation: Invalidation::Nothing,
        }
    }

    pub(crate) fn invalidate_local(reason: SkipReason, folder: Option<PathBuf>) -> Outcome {
        Outcome::Skipped {
            reason,
            invalidation: folder.map_or(Invalidation::Nothing, Invalidation::Local),
        }
    }

    pub(crate) fn remote_changed() -> Outcome {
        Outcome::Skipped {
            reason: SkipReason::RemoteChanged,
            invalidation: Invalidation::Remote,
        }
    }
}

pub(crate) struct Effect {
    pub(crate) changes: Changes,
    pub(crate) path: Option<PathBuf>,
    pub(crate) state: ItemState,
    pub(crate) conflict: Option<PathBuf>,
}

impl Effect {
    pub(crate) fn new(changes: Changes) -> Effect {
        Effect {
            changes,
            path: None,
            state: ItemState::Synced,
            conflict: None,
        }
    }

    pub(crate) fn with_path(mut self, path: impl Into<PathBuf>) -> Effect {
        self.path = Some(path.into());
        self
    }

    pub(crate) fn trashed(mut self) -> Effect {
        self.state = ItemState::Trashed;
        self
    }

    pub(crate) fn conflicted(mut self, copy: PathBuf) -> Effect {
        self.state = ItemState::Conflicted { copy: copy.clone() };
        self.conflict = Some(copy);
        self
    }
}
