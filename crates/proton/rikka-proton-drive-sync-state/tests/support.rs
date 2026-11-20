use rikka_proton_drive_sync_state::{Kind, LocalItem, RemoteItem, State, SyncedItem};

pub(crate) const LOCAL_ROOT: u64 = 1;
pub(crate) const REMOTE_ROOT: &str = "root";

pub(crate) struct Tree {
    pub(crate) local: Vec<LocalItem>,
    pub(crate) remote: Vec<RemoteItem>,
    pub(crate) synced: Vec<SyncedItem>,
}

impl Tree {
    pub(crate) fn synced() -> Tree {
        let mut tree = Tree {
            local: Vec::new(),
            remote: Vec::new(),
            synced: Vec::new(),
        };
        tree.add_synced(
            1,
            Kind::Folder,
            (10, LOCAL_ROOT),
            ("d", REMOTE_ROOT),
            "docs",
            None,
        );
        tree.add_synced(2, Kind::File, (11, 10), ("a", "d"), "a.txt", Some("aaa"));
        tree.add_synced(
            3,
            Kind::File,
            (12, LOCAL_ROOT),
            ("b", REMOTE_ROOT),
            "b.txt",
            Some("bbb"),
        );
        tree
    }

    pub(crate) fn add_synced(
        &mut self,
        id: i64,
        kind: Kind,
        (local, local_parent): (u64, u64),
        (remote, remote_parent): (&str, &str),
        name: &str,
        sha1: Option<&str>,
    ) {
        let revision = sha1.map(|_| "r1".to_owned());
        self.local
            .push(local_item(local, local_parent, name, kind, sha1));
        self.remote.push(RemoteItem {
            id: remote.to_owned(),
            parent: remote_parent.to_owned(),
            name: name.to_owned(),
            kind,
            revision: revision.clone(),
            sha1: sha1.map(str::to_owned),
            size: 3,
            modified: 1_700_000_000,
        });
        self.synced.push(SyncedItem {
            id,
            kind,
            local,
            local_parent,
            local_name: name.to_owned(),
            remote: remote.to_owned(),
            remote_parent: remote_parent.to_owned(),
            remote_name: name.to_owned(),
            sha1: sha1.map(str::to_owned),
            size: 3,
            revision,
        });
    }

    pub(crate) fn local(&mut self, id: u64) -> &mut LocalItem {
        self.local.iter_mut().find(|item| item.id == id).unwrap()
    }

    pub(crate) fn remote(&mut self, id: &str) -> &mut RemoteItem {
        self.remote.iter_mut().find(|item| item.id == id).unwrap()
    }

    pub(crate) fn drop_local(&mut self, id: u64) {
        self.local.retain(|item| item.id != id);
    }

    pub(crate) fn drop_remote(&mut self, id: &str) {
        self.remote.retain(|item| item.id != id);
    }

    pub(crate) fn add_local(&mut self, id: u64, parent: u64, name: &str, sha1: &str) {
        self.local
            .push(local_item(id, parent, name, Kind::File, Some(sha1)));
    }

    pub(crate) fn add_remote(&mut self, id: &str, parent: &str, name: &str, sha1: &str) {
        self.remote.push(RemoteItem {
            id: id.to_owned(),
            parent: parent.to_owned(),
            name: name.to_owned(),
            kind: Kind::File,
            revision: Some("r1".to_owned()),
            sha1: Some(sha1.to_owned()),
            size: 3,
            modified: 1_700_000_000,
        });
    }

    pub(crate) fn state(self) -> State {
        State::new(LOCAL_ROOT, REMOTE_ROOT).with_items(self.local, self.remote, self.synced)
    }
}

pub(crate) fn local_item(
    id: u64,
    parent: u64,
    name: &str,
    kind: Kind,
    sha1: Option<&str>,
) -> LocalItem {
    LocalItem {
        id,
        parent,
        name: name.to_owned(),
        kind,
        size: 3,
        modified: 1_700_000_000_000_000_000,
        sha1: sha1.map(str::to_owned),
    }
}
