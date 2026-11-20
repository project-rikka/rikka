use std::collections::{HashMap, HashSet};

use rikka_proton_api::offload;
use rikka_proton_drive::{DriveClient, DriveEvent, EventAction, Link, LinkId, Node};
use rikka_proton_drive_sync_state::{Changes, Kind, RemoteItem, State};

use crate::error::Result;
use crate::remote::tree::{RemoteRoot, remote_item};

pub(crate) enum RemoteUpdate {
    Changes(Changes),
    RootRemoved,
}

pub(crate) async fn apply_events(
    remote: &mut RemoteRoot,
    client: &DriveClient,
    state: &State,
    events: &[DriveEvent],
) -> Result<RemoteUpdate> {
    let root = remote.root_id().to_owned();
    let mut latest: HashMap<&str, &DriveEvent> = HashMap::new();
    let mut order = Vec::new();
    for event in events {
        let id = event.link().id().as_str();
        if latest.insert(id, event).is_none() {
            order.push(id);
        }
    }
    if let Some(event) = latest.get(root.as_str())
        && (event.action() == EventAction::Delete || event.link().is_trashed())
    {
        return Ok(RemoteUpdate::RootRemoved);
    }

    let mut tree: HashMap<String, RemoteItem> = state.remote().clone();
    let mut relevant: HashSet<&str> = order
        .iter()
        .copied()
        .filter(|id| *id != root && tree.contains_key(*id))
        .collect();
    loop {
        let before = relevant.len();
        for id in &order {
            let parent = latest[id].link().parent_id().map(LinkId::as_str);
            if *id != root
                && parent.is_some_and(|parent| {
                    parent == root
                        || tree
                            .get(parent)
                            .is_some_and(|item| item.kind == Kind::Folder)
                        || relevant.contains(parent)
                })
            {
                relevant.insert(id);
            }
        }
        if relevant.len() == before {
            break;
        }
    }

    let mut to_fetch = Vec::new();
    for id in order.iter().copied().filter(|id| relevant.contains(id)) {
        let event = latest[id];
        if event.action() == EventAction::Delete || event.link().is_trashed() {
            remove_subtree(&mut tree, remote, id);
        } else {
            to_fetch.push(LinkId::from(id));
        }
    }

    if !to_fetch.is_empty() {
        let links = client.links(remote.volume(), &to_fetch).await?;
        insert_links(remote, client, state, &mut tree, &root, links).await?;
    }

    let mut changes = Changes::default();
    for (id, item) in &tree {
        if state.remote().get(id) != Some(item) {
            changes.remote.push(item.clone());
        }
    }
    changes.remote_removed = state
        .remote()
        .keys()
        .filter(|id| !tree.contains_key(*id))
        .cloned()
        .collect();
    Ok(RemoteUpdate::Changes(changes))
}

async fn insert_links(
    remote: &mut RemoteRoot,
    client: &DriveClient,
    state: &State,
    tree: &mut HashMap<String, RemoteItem>,
    root: &str,
    mut pending: Vec<Link>,
) -> Result<()> {
    loop {
        let (ready, waiting): (Vec<Link>, Vec<Link>) = pending.into_iter().partition(|link| {
            link.parent_id().is_some_and(|parent| {
                parent.as_str() == root
                    || tree
                        .get(parent.as_str())
                        .is_some_and(|item| item.kind == Kind::Folder)
            })
        });
        pending = waiting;
        if ready.is_empty() {
            break;
        }

        let mut by_parent: HashMap<String, Vec<Link>> = HashMap::new();
        for link in ready {
            let parent = link
                .parent_id()
                .map(|parent| parent.as_str().to_owned())
                .unwrap_or_default();
            by_parent.entry(parent).or_default().push(link);
        }
        for (parent, links) in by_parent {
            let folder = remote.folder(client, state, &parent).await?;
            let opener = client.clone();
            let nodes: Vec<(LinkId, rikka_proton_drive::Result<Node>)> = offload(move || {
                links
                    .into_iter()
                    .map(|link| (link.id().clone(), opener.decrypt_link(link, &folder)))
                    .collect()
            })
            .await;
            for (id, node) in nodes {
                let node = match node {
                    Ok(node) => node,
                    Err(error) => {
                        tracing::warn!(link = %id, %error, "failed to open changed remote item");
                        continue;
                    }
                };
                let Some(item) = remote_item(&node) else {
                    remove_subtree(tree, remote, id.as_str());
                    continue;
                };
                let newly_known = !tree
                    .get(id.as_str())
                    .is_some_and(|child| child.kind == Kind::Folder);
                tree.insert(item.id.clone(), item);
                if let Node::Folder(folder) = node {
                    remote.cache_folder(folder.clone());
                    if newly_known {
                        for inside in remote.enumerate_under(client, folder).await? {
                            tree.insert(inside.id.clone(), inside);
                        }
                    }
                }
            }
        }
    }

    for link in pending {
        remove_subtree(tree, remote, link.id().as_str());
    }
    Ok(())
}

fn remove_subtree(tree: &mut HashMap<String, RemoteItem>, remote: &mut RemoteRoot, id: &str) {
    for removed in subtree(tree, id) {
        tree.remove(&removed);
        remote.evict_folder(&removed);
    }
}

fn subtree(tree: &HashMap<String, RemoteItem>, id: &str) -> Vec<String> {
    if !tree.contains_key(id) {
        return Vec::new();
    }
    let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
    for item in tree.values() {
        children
            .entry(item.parent.as_str())
            .or_default()
            .push(item.id.as_str());
    }
    let mut nodes = vec![id];
    let mut next = 0;
    while let Some(&node) = nodes.get(next) {
        if let Some(child_ids) = children.get(node) {
            nodes.extend(child_ids.iter().copied());
        }
        next += 1;
    }
    nodes.into_iter().map(str::to_owned).collect()
}
