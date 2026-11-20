use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    File,
    Folder,
}

impl Kind {
    pub(crate) fn code(self) -> i64 {
        match self {
            Kind::File => 0,
            Kind::Folder => 1,
        }
    }

    pub(crate) fn from_code(code: i64) -> Kind {
        match code {
            1 => Kind::Folder,
            _ => Kind::File,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalItem {
    pub id: u64,
    pub parent: u64,
    pub name: String,
    pub kind: Kind,
    pub size: u64,
    pub modified: i64,
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteItem {
    pub id: String,
    pub parent: String,
    pub name: String,
    pub kind: Kind,
    pub revision: Option<String>,
    pub sha1: Option<String>,
    pub size: u64,
    pub modified: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedItem {
    pub id: i64,
    pub kind: Kind,
    pub local: u64,
    pub local_parent: u64,
    pub local_name: String,
    pub remote: String,
    pub remote_parent: String,
    pub remote_name: String,
    pub sha1: Option<String>,
    pub size: u64,
    pub revision: Option<String>,
}

impl SyncedItem {
    #[must_use]
    pub fn from_sides(id: i64, local: &LocalItem, remote: &RemoteItem) -> SyncedItem {
        SyncedItem {
            id,
            kind: local.kind,
            local: local.id,
            local_parent: local.parent,
            local_name: local.name.clone(),
            remote: remote.id.clone(),
            remote_parent: remote.parent.clone(),
            remote_name: remote.name.clone(),
            sha1: remote.sha1.clone().or_else(|| local.sha1.clone()),
            size: local.size,
            revision: remote.revision.clone(),
        }
    }
}
