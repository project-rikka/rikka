use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use rikka_proton_drive_sync_state::{Operation, Root, RootId, State};

use crate::remote::tree::RemoteRoot;
use crate::status::{Problem, ProblemKind};

pub(crate) struct SyncRoot {
    pub(crate) id: RootId,
    pub(crate) record: Root,
    pub(crate) state: State,
    pub(crate) remote: Option<RemoteRoot>,
    pub(crate) local_missing: bool,
    pub(crate) enumerated: bool,
    pub(crate) dirty: HashSet<PathBuf>,
    pub(crate) rescan: bool,
    pub(crate) invalid: BTreeSet<PathBuf>,
    pub(crate) unreadable: BTreeSet<PathBuf>,
}

impl SyncRoot {
    pub(crate) fn new(
        id: RootId,
        record: Root,
        state: State,
        remote: Option<RemoteRoot>,
        enumerated: bool,
    ) -> SyncRoot {
        SyncRoot {
            id,
            record,
            state,
            remote,
            local_missing: false,
            enumerated,
            dirty: HashSet::new(),
            rescan: true,
            invalid: BTreeSet::new(),
            unreadable: BTreeSet::new(),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.record.local_path
    }

    pub(crate) fn is_healthy(&self) -> bool {
        self.remote.is_some() && !self.local_missing
    }

    pub(crate) fn local_path(&self, id: u64) -> Option<PathBuf> {
        if id == self.state.local_root() {
            return Some(self.path().to_owned());
        }
        self.state
            .local_path(id)
            .map(|relative| self.path().join(relative))
    }

    pub(crate) fn remote_path(&self, id: &str) -> PathBuf {
        self.state.remote_path(id).map_or_else(
            || self.path().to_owned(),
            |relative| self.path().join(relative),
        )
    }

    pub(crate) fn synced_path(&self, synced: i64) -> PathBuf {
        match self.state.synced().get(&synced) {
            Some(row) => self
                .local_path(row.local)
                .unwrap_or_else(|| self.remote_path(&row.remote)),
            None => self.path().to_owned(),
        }
    }

    pub(crate) fn operation_path(&self, operation: &Operation) -> PathBuf {
        match operation {
            Operation::Link { local, .. }
            | Operation::SetAside { local, .. }
            | Operation::CreateRemoteFolder { local, .. }
            | Operation::Upload { local, .. } => self
                .local_path(*local)
                .unwrap_or_else(|| self.path().to_owned()),
            Operation::CreateLocalFolder { parent, name, .. }
            | Operation::Download { parent, name, .. } => self
                .local_path(*parent)
                .unwrap_or_else(|| self.path().to_owned())
                .join(name),
            Operation::Rebind { synced, .. }
            | Operation::Refresh { synced }
            | Operation::Forget { synced }
            | Operation::MoveRemote { synced, .. }
            | Operation::MoveLocal { synced, .. }
            | Operation::TrashRemote { synced }
            | Operation::TrashLocal { synced }
            | Operation::UploadRevision { synced, .. }
            | Operation::DownloadRevision { synced, .. } => self.synced_path(*synced),
        }
    }

    pub(crate) fn operation_size(&self, operation: &Operation) -> u64 {
        match operation {
            Operation::Upload { local, .. } => {
                self.state.local().get(local).map_or(0, |item| item.size)
            }
            Operation::Download { remote, .. } => {
                self.state.remote().get(remote).map_or(0, |item| item.size)
            }
            Operation::UploadRevision { synced, .. } => self
                .state
                .synced()
                .get(synced)
                .and_then(|row| self.state.local().get(&row.local))
                .map_or(0, |item| item.size),
            Operation::DownloadRevision { synced, .. } => self
                .state
                .synced()
                .get(synced)
                .and_then(|row| self.state.remote().get(&row.remote))
                .map_or(0, |item| item.size),
            _ => 0,
        }
    }

    pub(crate) fn problems(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        if self.local_missing {
            problems.push(Problem::new(
                self.path(),
                ProblemKind::LocalFolderMissing,
                "folder is missing",
            ));
        }
        if self.remote.is_none() {
            problems.push(Problem::new(
                self.path(),
                ProblemKind::RemoteFolderRemoved,
                "folder was removed from Proton Drive",
            ));
        }
        problems.extend(self.invalid.iter().map(|path| {
            Problem::new(
                path.clone(),
                ProblemKind::InvalidName,
                "name is not valid UTF-8",
            )
        }));
        problems.extend(
            self.unreadable.iter().map(|path| {
                Problem::new(path.clone(), ProblemKind::Unreadable, "item is unreadable")
            }),
        );
        problems
    }
}
