use rikka_sync::Mutex;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use futures_util::lock::{Mutex as AsyncMutex, OwnedMutexGuard};

use crate::crypto::{FolderKey, ShareKeys};
use crate::identifier::{FolderId, GroupId, ShareId};

#[derive(Default)]
pub(crate) struct Keyring {
    shares: Mutex<HashMap<ShareId, Arc<AsyncMutex<ShareRing>>>>,
    groups: Mutex<HashMap<ShareId, Option<GroupId>>>,
}

impl Keyring {
    pub(crate) async fn ring(&self, share: &ShareId) -> OwnedMutexGuard<ShareRing> {
        let share_ring = {
            let mut shares = self.shares.lock();
            Arc::clone(shares.entry(share.clone()).or_default())
        };
        share_ring.lock_owned().await
    }

    pub(crate) fn set_share_group(&self, share: &ShareId, group: Option<GroupId>) {
        self.groups.lock().insert(share.clone(), group);
    }

    pub(crate) fn cached_group(&self, share: &ShareId) -> Option<Option<GroupId>> {
        self.groups.lock().get(share).cloned()
    }

    pub(crate) fn remove_share(&self, share: &ShareId) {
        self.shares.lock().remove(share);
        self.groups.lock().remove(share);
    }
}

impl fmt::Debug for Keyring {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("Keyring").finish_non_exhaustive()
    }
}

#[derive(Default)]
pub(crate) struct ShareRing {
    keys: Option<ShareKeys>,
    folders: HashMap<FolderId, FolderKey>,
}

impl ShareRing {
    pub(crate) fn keys_for_rotations(&self, rotations: &[i64]) -> Option<&ShareKeys> {
        self.keys.as_ref().filter(|keys| {
            rotations
                .iter()
                .all(|rotation| keys.get(*rotation).is_some())
        })
    }

    pub(crate) fn set_keys(&mut self, keys: ShareKeys) {
        self.keys = Some(keys);
    }

    pub(crate) fn folders_containing(
        &self,
        folder_ids: &[&FolderId],
    ) -> Option<&HashMap<FolderId, FolderKey>> {
        folder_ids
            .iter()
            .all(|folder| self.folders.contains_key(*folder))
            .then_some(&self.folders)
    }

    pub(crate) fn set_folders(&mut self, folders: HashMap<FolderId, FolderKey>) {
        self.folders = folders;
    }

    pub(crate) fn insert_folder(&mut self, folder: FolderId, key: FolderKey) {
        self.folders.insert(folder, key);
    }

    pub(crate) fn remove_folders(&mut self, folders: &[FolderId]) {
        for folder in folders {
            self.folders.remove(folder);
        }
    }
}
