use std::collections::HashMap;

use futures_util::{StreamExt as _, stream};
use rikka_proton_drive::{
    DriveClient, Error as DriveError, File, Folder, Link, LinkId, LinkState, Node, ShareKeys,
    VolumeId,
};
use rikka_proton_drive_sync_state::{Kind, RemoteItem, State, is_ignored};

use crate::error::Result;

const CONCURRENT_LISTINGS: usize = 4;
const MAX_DEPTH: usize = 512;

pub(crate) struct RemoteRoot {
    volume: VolumeId,
    share: ShareKeys,
    root: Folder,
    folders: HashMap<String, Folder>,
}

impl RemoteRoot {
    pub(crate) fn new(volume: VolumeId, share: ShareKeys, root: Folder) -> RemoteRoot {
        let mut folders = HashMap::new();
        folders.insert(root.id().as_str().to_owned(), root.clone());
        RemoteRoot {
            volume,
            share,
            root,
            folders,
        }
    }

    pub(crate) fn volume(&self) -> &VolumeId {
        &self.volume
    }

    pub(crate) fn share(&self) -> &ShareKeys {
        &self.share
    }

    pub(crate) fn root_id(&self) -> &str {
        self.root.id().as_str()
    }

    pub(crate) fn cache_folder(&mut self, folder: Folder) {
        self.folders.insert(folder.id().as_str().to_owned(), folder);
    }

    pub(crate) fn evict_folder(&mut self, id: &str) {
        if id != self.root_id() {
            self.folders.remove(id);
        }
    }

    pub(crate) async fn folder(
        &mut self,
        client: &DriveClient,
        state: &State,
        id: &str,
    ) -> Result<Folder> {
        let mut chain = Vec::new();
        let mut current = id.to_owned();
        while !self.folders.contains_key(&current) {
            let parent = state
                .remote()
                .get(&current)
                .map(|item| item.parent.clone())
                .ok_or_else(|| DriveError::LinkNotFound(LinkId::from(current.as_str())))?;
            chain.push(current);
            current = parent;
            if chain.len() > MAX_DEPTH {
                return Err(DriveError::LinkNotFound(LinkId::from(id)).into());
            }
        }
        let mut folder = self.folders[&current].clone();
        for link in chain.into_iter().rev() {
            folder = client
                .folder(&self.volume, &LinkId::from(link.as_str()), &folder)
                .await?;
            self.cache_folder(folder.clone());
        }
        Ok(folder)
    }

    pub(crate) async fn node(
        &mut self,
        client: &DriveClient,
        state: &State,
        id: &str,
    ) -> Result<(Folder, Node)> {
        let parent = state
            .remote()
            .get(id)
            .map(|item| item.parent.clone())
            .ok_or_else(|| DriveError::LinkNotFound(LinkId::from(id)))?;
        let folder = self.folder(client, state, &parent).await?;
        let node = client
            .node(&self.volume, &LinkId::from(id), &folder)
            .await?;
        if let Node::Folder(folder_node) = &node {
            self.cache_folder(folder_node.clone());
        }
        Ok((folder, node))
    }

    pub(crate) async fn file(
        &mut self,
        client: &DriveClient,
        state: &State,
        id: &str,
    ) -> Result<(Folder, File)> {
        let (folder, node) = self.node(client, state, id).await?;
        match node.into_file() {
            Some(file) => Ok((folder, file)),
            None => Err(DriveError::NotAFile(LinkId::from(id)).into()),
        }
    }

    pub(crate) async fn enumerate(&mut self, client: &DriveClient) -> Result<Vec<RemoteItem>> {
        let root = self.root.clone();
        self.enumerate_under(client, root).await
    }

    pub(crate) async fn enumerate_under(
        &mut self,
        client: &DriveClient,
        base: Folder,
    ) -> Result<Vec<RemoteItem>> {
        let mut found = Vec::new();
        let mut level = vec![base];
        while !level.is_empty() {
            let listed: Vec<Result<Vec<Node>, DriveError>> = stream::iter(level)
                .map(|folder| {
                    let (client, volume) = (client.clone(), self.volume.clone());
                    async move { client.children(&volume, &folder).await }
                })
                .buffer_unordered(CONCURRENT_LISTINGS)
                .collect()
                .await;
            let mut next = Vec::new();
            for nodes in listed {
                for node in nodes? {
                    let Some(item) = remote_item(&node) else {
                        continue;
                    };
                    if let Node::Folder(folder) = node {
                        self.cache_folder(folder.clone());
                        next.push(folder);
                    }
                    found.push(item);
                }
            }
            level = next;
        }
        Ok(found)
    }
}

pub(crate) fn remote_item(node: &Node) -> Option<RemoteItem> {
    let link = node.link();
    if link.state() != LinkState::Active || is_ignored(node.name()) {
        return None;
    }
    let parent = link.parent_id()?.as_str().to_owned();
    match node {
        Node::Folder(folder) if !folder.is_album() => Some(RemoteItem {
            id: link.id().as_str().to_owned(),
            parent,
            name: node.name().to_owned(),
            kind: Kind::Folder,
            revision: None,
            sha1: None,
            size: 0,
            modified: link_modified_seconds(link),
        }),
        Node::File(file) if !file.is_proton_document() => {
            let revision = file.active_revision()?.id().as_str().to_owned();
            let attributes = file.attributes();
            Some(RemoteItem {
                id: link.id().as_str().to_owned(),
                parent,
                name: node.name().to_owned(),
                kind: Kind::File,
                revision: Some(revision),
                sha1: file.sha1().map(str::to_ascii_lowercase),
                size: attributes
                    .and_then(|current_revision| current_revision.size())
                    .unwrap_or_else(|| u64::try_from(file.size()).unwrap_or_default()),
                modified: attributes
                    .and_then(|current_revision| current_revision.modified_at())
                    .or(link.modified_at())
                    .map_or(0, |time| time.as_second()),
            })
        }
        _ => None,
    }
}

fn link_modified_seconds(link: &Link) -> i64 {
    link.modified_at().map_or(0, |time| time.as_second())
}

pub(crate) async fn open_folder(
    client: &DriveClient,
    volume: &VolumeId,
    base: &Folder,
    id: &LinkId,
) -> Result<Option<Folder>> {
    let mut chain = Vec::new();
    let mut current = id.clone();
    while &current != base.id() {
        let link = match client.link(volume, &current).await {
            Ok(link) => link,
            Err(DriveError::LinkNotFound(_)) => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let Some(parent) = link.parent_id().cloned() else {
            return Ok(None);
        };
        if link.state() != LinkState::Active || chain.len() > MAX_DEPTH {
            return Ok(None);
        }
        chain.push(link);
        current = parent;
    }
    let mut folder = base.clone();
    for link in chain.into_iter().rev() {
        let id = link.id().clone();
        folder = client
            .decrypt_link(link, &folder)?
            .into_folder()
            .ok_or(DriveError::NotAFolder(id))?;
    }
    Ok(Some(folder))
}
