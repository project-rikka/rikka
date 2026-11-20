use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use rikka_proton_api::{ResponseCode, offload};
use rikka_proton_drive::{
    DriveClient, Error as DriveError, Folder, LinkId, Node, Parent, ShareKeys, VolumeId,
};
use rikka_proton_drive_sync_state::{
    Changes, Entry, Kind, LocalItem, Operation, RemoteItem, SyncedItem, is_ignored,
};

use crate::engine::Engine;
use crate::engine::effect::{Effect, Outcome, SkipReason};
use crate::engine::transfer::Prepared;
use crate::error::{Error, Result};
use crate::local::files::{available_local_name, modified, move_to_available_name, recycle};
use crate::remote::tree::remote_item;

const CREATE_FOLDER_ATTEMPTS: u32 = 8;

impl Engine {
    pub(super) async fn perform(&mut self, index: usize, entry: &Entry) -> Result<Outcome> {
        match &entry.operation {
            Operation::Link { local, remote } => self.link(index, *local, remote),
            Operation::Rebind {
                synced,
                local,
                remote,
            } => self.rebind(index, *synced, *local, remote.as_deref()),
            Operation::Refresh { synced } => self.refresh(index, *synced),
            Operation::Forget { synced } => Ok(Outcome::Done(Effect::new(Changes {
                synced_removed: vec![*synced],
                ..Changes::default()
            }))),
            Operation::SetAside {
                local,
                name,
                forget,
                ..
            } => self.set_aside(index, entry, *local, name, *forget).await,
            Operation::MoveRemote {
                synced,
                parent,
                name,
            } => self.move_remote(index, *synced, parent, name).await,
            Operation::MoveLocal {
                synced,
                parent,
                name,
            } => self.move_local(index, *synced, *parent, name),
            Operation::CreateRemoteFolder {
                local,
                parent,
                name,
            } => self.create_remote_folder(index, *local, parent, name).await,
            Operation::CreateLocalFolder {
                remote,
                parent,
                name,
            } => self.create_local_folder(index, remote, *parent, name),
            Operation::TrashRemote { synced } => self.trash_remote(index, *synced).await,
            Operation::TrashLocal { synced } => self.trash_local(index, *synced).await,
            Operation::Upload { .. }
            | Operation::UploadRevision { .. }
            | Operation::Download { .. }
            | Operation::DownloadRevision { .. } => match self.prepare(index, entry).await? {
                Prepared::Ready(job) => job.run().await,
                Prepared::Skipped(outcome) => Ok(*outcome),
            },
        }
    }

    fn link(&mut self, index: usize, local: u64, remote: &str) -> Result<Outcome> {
        let root = &mut self.roots[index];
        let (Some(local), Some(remote)) = (
            root.state.local().get(&local).cloned(),
            root.state.remote().get(remote).cloned(),
        ) else {
            return Ok(Outcome::skipped(SkipReason::LinkTargetMissing));
        };
        let id = root.state.allocate_synced_id();
        Ok(Outcome::Done(Effect::new(Changes {
            synced: vec![SyncedItem::from_sides(id, &local, &remote)],
            ..Changes::default()
        })))
    }

    fn rebind(
        &mut self,
        index: usize,
        synced: i64,
        local: Option<u64>,
        remote: Option<&str>,
    ) -> Result<Outcome> {
        let root = &self.roots[index];
        let Some(mut row) = root.state.synced().get(&synced).cloned() else {
            return Ok(Outcome::skipped(SkipReason::SyncedItemMissing));
        };
        if let Some(local) = local {
            row.local = local;
        }
        if let Some(remote) = remote {
            remote.clone_into(&mut row.remote);
        }
        Ok(Outcome::Done(Effect::new(Changes {
            synced: vec![row],
            ..Changes::default()
        })))
    }

    fn refresh(&mut self, index: usize, synced: i64) -> Result<Outcome> {
        let root = &self.roots[index];
        let Some(row) = root.state.synced().get(&synced) else {
            return Ok(Outcome::skipped(SkipReason::SyncedItemMissing));
        };
        let (Some(local), Some(remote)) = (
            root.state.local().get(&row.local),
            root.state.remote().get(&row.remote),
        ) else {
            return Ok(Outcome::skipped(SkipReason::SyncSideMissing));
        };
        Ok(Outcome::Done(Effect::new(Changes {
            synced: vec![SyncedItem::from_sides(row.id, local, remote)],
            ..Changes::default()
        })))
    }

    async fn set_aside(
        &mut self,
        index: usize,
        entry: &Entry,
        local: u64,
        name: &str,
        forget: Option<i64>,
    ) -> Result<Outcome> {
        let root = &self.roots[index];
        let (Some(item), Some(from)) = (
            root.state.local().get(&local).cloned(),
            root.local_path(local),
        ) else {
            return Ok(Outcome::skipped(SkipReason::ItemMissing));
        };
        let Some(folder) = from.parent().map(Path::to_owned) else {
            return Ok(Outcome::skipped(SkipReason::NoParentFolder));
        };
        if !std::fs::symlink_metadata(&from).is_ok_and(|metadata| metadata.ino() == local) {
            return Ok(Outcome::invalidate_local(
                SkipReason::MovedConcurrently,
                Some(folder),
            ));
        }
        let candidate = available_local_name(&folder, name).map_err(Error::io(&folder))?;
        let artifact = folder.join(&candidate);
        let id = entry.id;
        let recorded = artifact.clone();
        self.persist
            .run(move |store| store.record_artifact(id, &recorded))
            .await?;
        let final_name =
            move_to_available_name(&from, &folder, &candidate).map_err(Error::io(&from))?;
        let copy = folder.join(&final_name);
        Ok(Outcome::Done(
            Effect::new(Changes {
                local: vec![LocalItem {
                    name: final_name,
                    ..item
                }],
                synced_removed: forget.into_iter().collect(),
                ..Changes::default()
            })
            .with_path(from)
            .conflicted(copy),
        ))
    }

    async fn move_remote(
        &mut self,
        index: usize,
        synced: i64,
        parent: &str,
        name: &str,
    ) -> Result<Outcome> {
        let client = self.client.clone();
        let root = &mut self.roots[index];
        let Some(row) = root.state.synced().get(&synced).cloned() else {
            return Ok(Outcome::skipped(SkipReason::SyncedItemMissing));
        };
        let (Some(local), Some(remote_item)) = (
            root.state.local().get(&row.local).cloned(),
            root.state.remote().get(&row.remote).cloned(),
        ) else {
            return Ok(Outcome::skipped(SkipReason::SyncSideMissing));
        };
        let path = root.local_path(row.local);
        let Some(remote) = root.remote.as_mut() else {
            return Ok(Outcome::skipped(SkipReason::RemoteFolderMissing));
        };
        let (from, node) = remote.node(&client, &root.state, &row.remote).await?;
        let to = remote.folder(&client, &root.state, parent).await?;
        let mover = RemoteMover {
            client: &client,
            volume: remote.volume(),
            share: remote.share(),
        };
        let final_name = mover.place(node, &from, &to, name).await?;
        let moved = RemoteItem {
            parent: parent.to_owned(),
            name: final_name.clone(),
            ..remote_item
        };
        let row = SyncedItem {
            local_parent: local.parent,
            local_name: local.name.clone(),
            remote_parent: parent.to_owned(),
            remote_name: final_name,
            ..row
        };
        let effect = Effect::new(Changes {
            remote: vec![moved],
            synced: vec![row],
            ..Changes::default()
        });
        Ok(Outcome::Done(match path {
            Some(path) => effect.with_path(path),
            None => effect,
        }))
    }

    fn move_local(
        &mut self,
        index: usize,
        synced: i64,
        parent: u64,
        name: &str,
    ) -> Result<Outcome> {
        let root = &self.roots[index];
        let Some(row) = root.state.synced().get(&synced).cloned() else {
            return Ok(Outcome::skipped(SkipReason::SyncedItemMissing));
        };
        let (Some(local), Some(remote)) = (
            root.state.local().get(&row.local).cloned(),
            root.state.remote().get(&row.remote).cloned(),
        ) else {
            return Ok(Outcome::skipped(SkipReason::SyncSideMissing));
        };
        let (Some(from), Some(folder)) = (root.local_path(row.local), root.local_path(parent))
        else {
            return Ok(Outcome::skipped(SkipReason::FolderMissing));
        };
        if !std::fs::symlink_metadata(&from).is_ok_and(|metadata| metadata.ino() == row.local) {
            return Ok(Outcome::invalidate_local(
                SkipReason::MovedConcurrently,
                from.parent().map(Path::to_owned),
            ));
        }
        let final_name = move_to_available_name(&from, &folder, name).map_err(Error::io(&from))?;
        let row = SyncedItem {
            local_parent: parent,
            local_name: final_name.clone(),
            remote_parent: remote.parent.clone(),
            remote_name: remote.name.clone(),
            ..row
        };
        Ok(Outcome::Done(
            Effect::new(Changes {
                local: vec![LocalItem {
                    parent,
                    name: final_name.clone(),
                    ..local
                }],
                synced: vec![row],
                ..Changes::default()
            })
            .with_path(folder.join(final_name)),
        ))
    }

    async fn create_remote_folder(
        &mut self,
        index: usize,
        local: u64,
        parent: &str,
        name: &str,
    ) -> Result<Outcome> {
        let client = self.client.clone();
        let root = &mut self.roots[index];
        let Some(local) = root.state.local().get(&local).cloned() else {
            return Ok(Outcome::skipped(SkipReason::FolderMissing));
        };
        let path = root.local_path(local.id);
        let Some(remote) = root.remote.as_mut() else {
            return Ok(Outcome::skipped(SkipReason::RemoteFolderMissing));
        };
        let parent_folder = remote.folder(&client, &root.state, parent).await?;
        let created = match client
            .create_folder(remote.share(), &parent_folder, name)
            .await
        {
            Ok(created) => created,
            Err(DriveError::Conflict { .. }) => {
                let available = client
                    .available_folder_name(remote.volume(), &parent_folder, name)
                    .await?;
                client
                    .create_folder(remote.share(), &parent_folder, &available)
                    .await?
            }
            Err(error) => return Err(error.into()),
        };
        remote.cache_folder(created.clone());
        let Some(item) = remote_item(&Node::Folder(created)) else {
            return Ok(Outcome::skipped(SkipReason::RemoteFolderUnreadable));
        };
        let id = root.state.allocate_synced_id();
        let effect = Effect::new(Changes {
            synced: vec![SyncedItem::from_sides(id, &local, &item)],
            remote: vec![item],
            ..Changes::default()
        });
        Ok(Outcome::Done(match path {
            Some(path) => effect.with_path(path),
            None => effect,
        }))
    }

    fn create_local_folder(
        &mut self,
        index: usize,
        remote: &str,
        parent: u64,
        name: &str,
    ) -> Result<Outcome> {
        let root = &mut self.roots[index];
        let Some(remote) = root.state.remote().get(remote).cloned() else {
            return Ok(Outcome::skipped(SkipReason::RemoteFolderMissing));
        };
        let Some(folder) = root.local_path(parent) else {
            return Ok(Outcome::skipped(SkipReason::FolderMissing));
        };
        let mut created = None;
        for _ in 0..CREATE_FOLDER_ATTEMPTS {
            let candidate = available_local_name(&folder, name).map_err(Error::io(&folder))?;
            let path = folder.join(&candidate);
            match std::fs::create_dir(&path) {
                Ok(()) => {
                    created = Some((candidate, path));
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(Error::io(&path)(error)),
            }
        }
        let Some((final_name, path)) = created else {
            return Ok(Outcome::skipped(SkipReason::NoAvailableName));
        };
        let metadata = std::fs::symlink_metadata(&path).map_err(Error::io(&path))?;
        let item = LocalItem {
            id: metadata.ino(),
            parent,
            name: final_name,
            kind: Kind::Folder,
            size: 0,
            modified: modified(&metadata),
            sha1: None,
        };
        let id = root.state.allocate_synced_id();
        Ok(Outcome::Done(
            Effect::new(Changes {
                synced: vec![SyncedItem::from_sides(id, &item, &remote)],
                local: vec![item],
                ..Changes::default()
            })
            .with_path(path),
        ))
    }

    async fn trash_remote(&mut self, index: usize, synced: i64) -> Result<Outcome> {
        let root = &mut self.roots[index];
        let Some(row) = root.state.synced().get(&synced).cloned() else {
            return Ok(Outcome::skipped(SkipReason::SyncedItemMissing));
        };
        let path = root.remote_path(&row.remote);
        let Some(remote) = root.remote.as_mut() else {
            return Ok(Outcome::skipped(SkipReason::RemoteFolderMissing));
        };
        let outcomes = self
            .client
            .trash(remote.volume(), &[LinkId::from(row.remote.as_str())])
            .await?;
        if let Some(refused) = outcomes
            .iter()
            .find(|outcome| !outcome.is_ok() && outcome.code() != ResponseCode::NotExists)
        {
            return Err(DriveError::Refused {
                code: refused.code(),
                message: refused.error().unwrap_or_default().to_owned(),
            }
            .into());
        }
        let subtree = root.state.remote_subtree(&row.remote);
        for id in &subtree {
            remote.evict_folder(id);
        }
        let rows = root
            .state
            .synced()
            .values()
            .filter(|synced_item| subtree.contains(&synced_item.remote))
            .map(|synced_item| synced_item.id)
            .collect();
        Ok(Outcome::Done(
            Effect::new(Changes {
                remote_removed: subtree,
                synced_removed: rows,
                ..Changes::default()
            })
            .with_path(path)
            .trashed(),
        ))
    }

    async fn trash_local(&mut self, index: usize, synced: i64) -> Result<Outcome> {
        let root = &self.roots[index];
        let Some(row) = root.state.synced().get(&synced).cloned() else {
            return Ok(Outcome::skipped(SkipReason::SyncedItemMissing));
        };
        let Some(path) = root.local_path(row.local) else {
            return Ok(Outcome::skipped(SkipReason::ItemMissing));
        };
        let subtree = root.state.local_subtree(row.local);
        let expected: Vec<LocalItem> = subtree
            .iter()
            .filter_map(|id| root.state.local().get(id).cloned())
            .collect();
        let root_path = root.path().to_owned();
        let checked = path.clone();
        let trashed = offload(move || {
            if !is_tree_unchanged(&checked, &expected) {
                return Ok(None);
            }
            recycle(&checked, &root_path).map(Some)
        })
        .await
        .map_err(Error::io(&path))?;
        if trashed.is_none() {
            return Ok(Outcome::invalidate_local(
                SkipReason::ChangedLocally,
                path.parent().map(Path::to_owned),
            ));
        }
        let rows = root
            .state
            .synced()
            .values()
            .filter(|synced_item| subtree.contains(&synced_item.local))
            .map(|synced_item| synced_item.id)
            .collect();
        Ok(Outcome::Done(
            Effect::new(Changes {
                local_removed: subtree,
                synced_removed: rows,
                ..Changes::default()
            })
            .with_path(path)
            .trashed(),
        ))
    }
}

fn is_tree_unchanged(path: &Path, expected: &[LocalItem]) -> bool {
    let known: std::collections::HashMap<u64, &LocalItem> =
        expected.iter().map(|item| (item.id, item)).collect();
    let mut pending = vec![path.to_owned()];
    while let Some(current) = pending.pop() {
        let Ok(metadata) = std::fs::symlink_metadata(&current) else {
            return false;
        };
        let Some(item) = known.get(&metadata.ino()) else {
            return false;
        };
        if metadata.is_dir() {
            let Ok(entries) = std::fs::read_dir(&current) else {
                return false;
            };
            for entry in entries.flatten() {
                let ignored = entry.file_name().to_str().is_some_and(is_ignored);
                if !ignored {
                    pending.push(entry.path());
                }
            }
        } else if item.kind != Kind::File
            || metadata.len() != item.size
            || modified(&metadata) != item.modified
        {
            return false;
        }
    }
    true
}

struct RemoteMover<'a> {
    client: &'a DriveClient,
    volume: &'a VolumeId,
    share: &'a ShareKeys,
}

impl RemoteMover<'_> {
    async fn place(
        &self,
        mut node: Node,
        from: &Folder,
        to: &Folder,
        name: &str,
    ) -> Result<String> {
        let mut current = node.name().to_owned();
        if from.id() == to.id() {
            if current != name {
                current = self.rename(from, &node, name).await?;
            }
            return Ok(current);
        }

        if current != name {
            current = self.rename(from, &node, name).await?;
            node = self.client.node(self.volume, node.id(), from).await?;
        }
        match self.client.move_node(self.share, &node, to).await {
            Ok(()) => Ok(current),
            Err(DriveError::Conflict { .. }) => {
                let available = self.available_name(to, &node, &current).await?;
                self.rename(from, &node, &available).await?;
                let node = self.client.node(self.volume, node.id(), from).await?;
                self.client.move_node(self.share, &node, to).await?;
                Ok(available)
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn rename(&self, folder: &Folder, node: &Node, name: &str) -> Result<String> {
        match self
            .client
            .rename(self.share, Parent::Folder(folder), node, name)
            .await
        {
            Ok(()) => Ok(name.to_owned()),
            Err(DriveError::Conflict { .. }) => {
                let available = self.available_name(folder, node, name).await?;
                self.client
                    .rename(self.share, Parent::Folder(folder), node, &available)
                    .await?;
                Ok(available)
            }
            Err(error) => Err(error.into()),
        }
    }

    async fn available_name(&self, folder: &Folder, node: &Node, name: &str) -> Result<String> {
        Ok(match node.is_folder() {
            true => {
                self.client
                    .available_folder_name(self.volume, folder, name)
                    .await?
            }
            false => {
                self.client
                    .available_name(self.volume, folder, name)
                    .await?
            }
        })
    }
}
