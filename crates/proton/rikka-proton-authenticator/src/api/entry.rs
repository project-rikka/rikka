use jiff::Timestamp;
use rikka_proton_cache::Wired;
use serde::{Deserialize, Serialize};

use crate::api::key::SyncKey;
use crate::entry::Entry;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteEntry {
    pub remote_id: String,
    pub revision: u32,
    pub entry: Entry,
    pub position: Option<u32>,
    pub created_at: Timestamp,
    pub modified_at: Timestamp,
}

impl RemoteEntry {
    pub(crate) fn decrypt(
        payload: &EntryPayload,
        keys: &[SyncKey],
        position: Option<u32>,
    ) -> Option<RemoteEntry> {
        let entry = keys
            .iter()
            .find_map(|key| key.key().open(&payload.content).ok())?;

        Some(RemoteEntry {
            remote_id: payload.id.clone(),
            revision: payload.revision,
            entry,
            position,
            created_at: payload.created_at,
            modified_at: payload.modified_at,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryUpdate {
    pub remote_id: String,
    pub revision: u32,
    pub entry: Entry,
}

impl EntryUpdate {
    #[must_use]
    pub fn new(remote_id: impl Into<String>, revision: u32, entry: Entry) -> EntryUpdate {
        EntryUpdate {
            remote_id: remote_id.into(),
            revision,
            entry,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct EntryPayload {
    #[serde(rename = "EntryID")]
    pub(crate) id: String,
    #[serde(rename = "Revision", default)]
    pub(crate) revision: u32,
    #[serde(rename = "Content")]
    pub(crate) content: String,
    #[serde(
        rename = "CreateTime",
        deserialize_with = "rikka_proton_api::unix_seconds::deserialize"
    )]
    pub(crate) created_at: Timestamp,
    #[serde(
        rename = "ModifyTime",
        deserialize_with = "rikka_proton_api::unix_seconds::deserialize"
    )]
    pub(crate) modified_at: Timestamp,
}

#[derive(Debug, Deserialize)]
pub(crate) struct EntriesPage {
    #[serde(rename = "Entries", default)]
    pub(crate) entries: Vec<Wired<EntryPayload>>,
    #[serde(rename = "LastID", default)]
    pub(crate) last_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OrderResult {
    #[serde(
        rename = "ModifyTime",
        deserialize_with = "rikka_proton_api::unix_seconds::deserialize"
    )]
    pub(crate) modified_at: Timestamp,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateEntryRequest {
    #[serde(rename = "AuthenticatorKeyID")]
    pub(crate) key_id: String,
    #[serde(rename = "Content")]
    pub(crate) content: String,
    #[serde(rename = "ContentFormatVersion")]
    pub(crate) content_format_version: u8,
}

#[derive(Debug, Serialize)]
pub(crate) struct CreateEntriesRequest {
    #[serde(rename = "Entries")]
    pub(crate) entries: Vec<CreateEntryRequest>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateEntryRequest {
    #[serde(rename = "AuthenticatorKeyID")]
    pub(crate) key_id: String,
    #[serde(rename = "EntryID")]
    pub(crate) entry_id: String,
    #[serde(rename = "Content")]
    pub(crate) content: String,
    #[serde(rename = "ContentFormatVersion")]
    pub(crate) content_format_version: u8,
    #[serde(rename = "LastRevision")]
    pub(crate) last_revision: u32,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateEntriesRequest {
    #[serde(rename = "Entries")]
    pub(crate) entries: Vec<UpdateEntryRequest>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DeleteEntriesRequest {
    #[serde(rename = "EntryIDs")]
    pub(crate) entry_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OrderEntriesRequest {
    #[serde(rename = "StartingPosition")]
    pub(crate) starting_position: u32,
    #[serde(rename = "Entries")]
    pub(crate) entry_ids: Vec<String>,
}
