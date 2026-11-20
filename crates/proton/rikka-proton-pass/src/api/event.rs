use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::api::item::ItemRevision;
use crate::api::share::ShareResponse;
use crate::identifier::{EventId, FolderId, ItemId, ShareId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareEventList {
    #[serde(
        rename = "UpdatedShare",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    updated_share: Option<ShareResponse>,
    #[serde(rename = "UpdatedItems", default)]
    updated_items: Vec<ItemRevision>,
    #[serde(rename = "DeletedItemIDs", default)]
    deleted_items: Vec<ItemId>,
    #[serde(
        rename = "NewKeyRotation",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    new_key_rotation: Option<i64>,
    #[serde(rename = "LatestEventID", default)]
    latest_event: EventId,
    #[serde(rename = "EventsPending", default)]
    pending: bool,
}

impl ShareEventList {
    #[must_use]
    pub fn updated_share(&self) -> Option<&ShareResponse> {
        self.updated_share.as_ref()
    }

    #[must_use]
    pub fn updated_items(&self) -> &[ItemRevision] {
        &self.updated_items
    }

    #[must_use]
    pub fn deleted_items(&self) -> &[ItemId] {
        &self.deleted_items
    }

    #[must_use]
    pub fn new_key_rotation(&self) -> Option<i64> {
        self.new_key_rotation
    }

    #[must_use]
    pub fn latest_event(&self) -> &EventId {
        &self.latest_event
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending
    }

    pub(crate) fn into_parts(self) -> (Option<ShareResponse>, Vec<ItemRevision>) {
        (self.updated_share, self.updated_items)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEventItem {
    #[serde(rename = "ShareID")]
    share_id: ShareId,
    #[serde(rename = "ItemID")]
    item_id: ItemId,
    #[serde(rename = "EventToken", default)]
    event_token: String,
}

impl UserEventItem {
    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn item_id(&self) -> &ItemId {
        &self.item_id
    }

    #[must_use]
    pub fn event_token(&self) -> &str {
        &self.event_token
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEventShare {
    #[serde(rename = "ShareID")]
    share_id: ShareId,
    #[serde(rename = "EventToken", default)]
    event_token: String,
}

impl UserEventShare {
    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn event_token(&self) -> &str {
        &self.event_token
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEventFolder {
    #[serde(rename = "ShareID")]
    share_id: ShareId,
    #[serde(rename = "FolderID")]
    folder_id: FolderId,
    #[serde(rename = "EventToken", default)]
    event_token: String,
}

impl UserEventFolder {
    #[must_use]
    pub fn share_id(&self) -> &ShareId {
        &self.share_id
    }

    #[must_use]
    pub fn folder_id(&self) -> &FolderId {
        &self.folder_id
    }

    #[must_use]
    pub fn event_token(&self) -> &str {
        &self.event_token
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEvents {
    #[serde(rename = "LastEventID", default)]
    last_event: EventId,
    #[serde(rename = "ItemsUpdated", default)]
    items_updated: Vec<UserEventItem>,
    #[serde(rename = "ItemsDeleted", default)]
    items_deleted: Vec<UserEventItem>,
    #[serde(rename = "AliasNoteChanged", default)]
    alias_notes_changed: Vec<UserEventItem>,
    #[serde(rename = "SharesCreated", default)]
    shares_created: Vec<UserEventShare>,
    #[serde(rename = "SharesUpdated", default)]
    shares_updated: Vec<UserEventShare>,
    #[serde(rename = "SharesDeleted", default)]
    shares_deleted: Vec<UserEventShare>,
    #[serde(rename = "FoldersUpdated", default)]
    folders_updated: Vec<UserEventFolder>,
    #[serde(rename = "FoldersDeleted", default)]
    folders_deleted: Vec<UserEventFolder>,
    #[serde(rename = "RefreshUser", default)]
    refresh_user: bool,
    #[serde(rename = "EventsPending", default)]
    pending: bool,
    #[serde(rename = "FullRefresh", default)]
    full_refresh: bool,
}

impl UserEvents {
    #[must_use]
    pub fn last_event(&self) -> &EventId {
        &self.last_event
    }

    #[must_use]
    pub fn items_updated(&self) -> &[UserEventItem] {
        &self.items_updated
    }

    #[must_use]
    pub fn items_deleted(&self) -> &[UserEventItem] {
        &self.items_deleted
    }

    #[must_use]
    pub fn alias_notes_changed(&self) -> &[UserEventItem] {
        &self.alias_notes_changed
    }

    #[must_use]
    pub fn shares_created(&self) -> &[UserEventShare] {
        &self.shares_created
    }

    #[must_use]
    pub fn shares_updated(&self) -> &[UserEventShare] {
        &self.shares_updated
    }

    #[must_use]
    pub fn shares_deleted(&self) -> &[UserEventShare] {
        &self.shares_deleted
    }

    #[must_use]
    pub fn folders_updated(&self) -> &[UserEventFolder] {
        &self.folders_updated
    }

    #[must_use]
    pub fn folders_deleted(&self) -> &[UserEventFolder] {
        &self.folders_deleted
    }

    #[must_use]
    pub fn refreshes_user(&self) -> bool {
        self.refresh_user
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.pending
    }

    #[must_use]
    pub fn is_full_refresh(&self) -> bool {
        self.full_refresh
    }

    #[must_use]
    pub fn changed_shares(&self) -> Vec<ShareId> {
        let deleted: BTreeSet<&ShareId> = self
            .shares_deleted
            .iter()
            .map(UserEventShare::share_id)
            .collect();
        let changed: BTreeSet<&ShareId> = self
            .shares_created
            .iter()
            .chain(&self.shares_updated)
            .map(UserEventShare::share_id)
            .chain(
                self.items_updated
                    .iter()
                    .chain(&self.items_deleted)
                    .chain(&self.alias_notes_changed)
                    .map(UserEventItem::share_id),
            )
            .chain(
                self.folders_updated
                    .iter()
                    .chain(&self.folders_deleted)
                    .map(UserEventFolder::share_id),
            )
            .filter(|share| !deleted.contains(share))
            .collect();
        changed.into_iter().cloned().collect()
    }
}
