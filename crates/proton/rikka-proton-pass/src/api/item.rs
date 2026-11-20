use jiff::Timestamp;
use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

use crate::identifier::{FolderId, ItemId, ShareId};

wire_enum! {
    pub enum ItemState: default Active {
        Active = 1,
        Trashed = 2,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRevision {
    #[serde(rename = "ItemID")]
    item_id: ItemId,
    #[serde(rename = "Revision", default)]
    revision: i64,
    #[serde(rename = "ContentFormatVersion", default)]
    content_format_version: i32,
    #[serde(rename = "KeyRotation", default)]
    key_rotation: i64,
    #[serde(rename = "Content", default)]
    content: String,
    #[serde(rename = "ItemKey", default, skip_serializing_if = "Option::is_none")]
    item_key: Option<String>,
    #[serde(rename = "State", default)]
    state: ItemState,
    #[serde(
        rename = "AliasEmail",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    alias_email: Option<String>,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(
        rename = "ModifyTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    modify_time: Timestamp,
    #[serde(
        rename = "LastUseTime",
        default,
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    last_use_time: Option<Timestamp>,
    #[serde(
        rename = "RevisionTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    revision_time: Timestamp,
    #[serde(rename = "Pinned", default)]
    pinned: bool,
    #[serde(
        rename = "PinTime",
        default,
        with = "rikka_proton_api::unix_seconds::optional",
        skip_serializing_if = "Option::is_none"
    )]
    pin_time: Option<Timestamp>,
    #[serde(rename = "Flags", default)]
    flags: i64,
    #[serde(rename = "ShareCount", default)]
    share_count: i64,
    #[serde(rename = "FolderID", default, skip_serializing_if = "Option::is_none")]
    folder_id: Option<FolderId>,
}

impl ItemRevision {
    #[must_use]
    pub fn item_id(&self) -> &ItemId {
        &self.item_id
    }

    #[must_use]
    pub fn revision(&self) -> i64 {
        self.revision
    }

    #[must_use]
    pub fn content_format_version(&self) -> i32 {
        self.content_format_version
    }

    #[must_use]
    pub fn key_rotation(&self) -> i64 {
        self.key_rotation
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    #[must_use]
    pub fn item_key(&self) -> Option<&str> {
        self.item_key.as_deref()
    }

    #[must_use]
    pub fn state(&self) -> ItemState {
        self.state
    }

    #[must_use]
    pub fn alias_email(&self) -> Option<&str> {
        self.alias_email.as_deref()
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn modify_time(&self) -> Timestamp {
        self.modify_time
    }

    #[must_use]
    pub fn last_use_time(&self) -> Option<Timestamp> {
        self.last_use_time
    }

    #[must_use]
    pub fn revision_time(&self) -> Timestamp {
        self.revision_time
    }

    #[must_use]
    pub fn is_pinned(&self) -> bool {
        self.pinned
    }

    #[must_use]
    pub fn pin_time(&self) -> Option<Timestamp> {
        self.pin_time
    }

    #[must_use]
    pub fn flags(&self) -> i64 {
        self.flags
    }

    #[must_use]
    pub fn share_count(&self) -> i64 {
        self.share_count
    }

    #[must_use]
    pub fn folder_id(&self) -> Option<&FolderId> {
        self.folder_id.as_ref()
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateItemRequest {
    #[serde(rename = "KeyRotation")]
    pub(crate) key_rotation: i64,
    #[serde(rename = "ContentFormatVersion")]
    pub(crate) content_format_version: i32,
    #[serde(rename = "Content")]
    pub(crate) content: String,
    #[serde(rename = "ItemKey")]
    pub(crate) item_key: String,
    #[serde(rename = "FolderID", default, skip_serializing_if = "Option::is_none")]
    pub(crate) folder_id: Option<FolderId>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UpdateItemRequest {
    #[serde(rename = "KeyRotation")]
    pub(crate) key_rotation: i64,
    #[serde(rename = "LastRevision")]
    pub(crate) last_revision: i64,
    #[serde(rename = "ContentFormatVersion")]
    pub(crate) content_format_version: i32,
    #[serde(rename = "Content")]
    pub(crate) content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemTarget {
    #[serde(rename = "ItemID")]
    item_id: ItemId,
    #[serde(rename = "Revision")]
    revision: i64,
}

impl ItemTarget {
    #[must_use]
    pub fn new(item_id: ItemId, revision: i64) -> ItemTarget {
        ItemTarget { item_id, revision }
    }

    #[must_use]
    pub fn item_id(&self) -> &ItemId {
        &self.item_id
    }

    #[must_use]
    pub fn revision(&self) -> i64 {
        self.revision
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrashedItem {
    #[serde(rename = "ItemID")]
    item_id: ItemId,
    #[serde(rename = "Revision", default)]
    revision: i64,
    #[serde(rename = "State", default)]
    state: ItemState,
    #[serde(
        rename = "ModifyTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    modify_time: Timestamp,
    #[serde(
        rename = "RevisionTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    revision_time: Timestamp,
    #[serde(rename = "Flags", default)]
    flags: i64,
}

impl TrashedItem {
    #[must_use]
    pub fn item_id(&self) -> &ItemId {
        &self.item_id
    }

    #[must_use]
    pub fn revision(&self) -> i64 {
        self.revision
    }

    #[must_use]
    pub fn state(&self) -> ItemState {
        self.state
    }

    #[must_use]
    pub fn modify_time(&self) -> Timestamp {
        self.modify_time
    }

    #[must_use]
    pub fn revision_time(&self) -> Timestamp {
        self.revision_time
    }

    #[must_use]
    pub fn flags(&self) -> i64 {
        self.flags
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemLatestKey {
    #[serde(rename = "KeyRotation")]
    rotation: i64,
    #[serde(rename = "Key")]
    key: String,
}

impl ItemLatestKey {
    pub(crate) fn new(rotation: i64, key: String) -> ItemLatestKey {
        ItemLatestKey { rotation, key }
    }

    #[must_use]
    pub fn rotation(&self) -> i64 {
        self.rotation
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ItemTargets {
    #[serde(rename = "Items")]
    pub(crate) items: Vec<ItemTarget>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct LastUsedRequest {
    #[serde(rename = "LastUseTime", with = "rikka_proton_api::unix_seconds")]
    pub(crate) last_use_time: Timestamp,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ItemFlagsRequest {
    #[serde(rename = "SkipHealthCheck", skip_serializing_if = "Option::is_none")]
    skip_health_check: Option<bool>,
    #[serde(
        rename = "SkipWeakPasswordCheck",
        skip_serializing_if = "Option::is_none"
    )]
    skip_weak_password_check: Option<bool>,
    #[serde(
        rename = "SkipCompromisedPasswordCheck",
        skip_serializing_if = "Option::is_none"
    )]
    skip_compromised_password_check: Option<bool>,
    #[serde(
        rename = "SkipReusedPasswordCheck",
        skip_serializing_if = "Option::is_none"
    )]
    skip_reused_password_check: Option<bool>,
    #[serde(rename = "Skip2FACheck", skip_serializing_if = "Option::is_none")]
    skip_two_factor_check: Option<bool>,
}

impl ItemFlagsRequest {
    #[must_use]
    pub fn new() -> ItemFlagsRequest {
        ItemFlagsRequest::default()
    }

    #[must_use]
    pub fn skip_health_check(mut self, skip: bool) -> ItemFlagsRequest {
        self.skip_health_check = Some(skip);
        self
    }

    #[must_use]
    pub fn skip_weak_password_check(mut self, skip: bool) -> ItemFlagsRequest {
        self.skip_weak_password_check = Some(skip);
        self
    }

    #[must_use]
    pub fn skip_compromised_password_check(mut self, skip: bool) -> ItemFlagsRequest {
        self.skip_compromised_password_check = Some(skip);
        self
    }

    #[must_use]
    pub fn skip_reused_password_check(mut self, skip: bool) -> ItemFlagsRequest {
        self.skip_reused_password_check = Some(skip);
        self
    }

    #[must_use]
    pub fn skip_two_factor_check(mut self, skip: bool) -> ItemFlagsRequest {
        self.skip_two_factor_check = Some(skip);
        self
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ItemPage {
    #[serde(rename = "RevisionsData", default)]
    pub(crate) revisions: Vec<ItemRevision>,
    #[serde(rename = "LastToken", default)]
    pub(crate) last_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MoveToShareRequest {
    #[serde(rename = "ShareID")]
    pub(crate) share_id: ShareId,
    #[serde(rename = "Items")]
    pub(crate) items: Vec<MoveToShareItem>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MoveToShareItem {
    #[serde(rename = "ItemID")]
    pub(crate) item_id: ItemId,
    #[serde(rename = "DestinationFolderID")]
    pub(crate) folder_id: Option<FolderId>,
    #[serde(rename = "ItemKeys")]
    pub(crate) keys: Vec<ItemLatestKey>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MoveToFolderRequest {
    #[serde(rename = "FolderID")]
    pub(crate) folder_id: Option<FolderId>,
    #[serde(rename = "Items")]
    pub(crate) items: Vec<MoveToFolderItem>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MoveToFolderItem {
    #[serde(rename = "ItemID")]
    pub(crate) item_id: ItemId,
    #[serde(rename = "ItemKeys")]
    pub(crate) keys: Vec<ItemLatestKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovedItem {
    #[serde(rename = "ItemID")]
    item_id: ItemId,
    #[serde(rename = "Revision", default)]
    revision: i64,
    #[serde(rename = "State", default)]
    state: ItemState,
    #[serde(rename = "Flags", default)]
    flags: i64,
    #[serde(rename = "FolderID", default, skip_serializing_if = "Option::is_none")]
    folder_id: Option<FolderId>,
    #[serde(
        rename = "ModifyTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    modify_time: Timestamp,
    #[serde(
        rename = "RevisionTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    revision_time: Timestamp,
}

impl MovedItem {
    #[must_use]
    pub fn item_id(&self) -> &ItemId {
        &self.item_id
    }

    #[must_use]
    pub fn revision(&self) -> i64 {
        self.revision
    }

    #[must_use]
    pub fn state(&self) -> ItemState {
        self.state
    }

    #[must_use]
    pub fn flags(&self) -> i64 {
        self.flags
    }

    #[must_use]
    pub fn folder_id(&self) -> Option<&FolderId> {
        self.folder_id.as_ref()
    }

    #[must_use]
    pub fn modify_time(&self) -> Timestamp {
        self.modify_time
    }

    #[must_use]
    pub fn revision_time(&self) -> Timestamp {
        self.revision_time
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ImportItemsRequest {
    #[serde(rename = "Items")]
    pub(crate) items: Vec<ImportItemRequest>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ImportItemRequest {
    #[serde(rename = "Item")]
    pub(crate) item: CreateItemRequest,
    #[serde(rename = "AliasEmail", skip_serializing_if = "Option::is_none")]
    pub(crate) alias_email: Option<String>,
    #[serde(rename = "Trashed")]
    pub(crate) trashed: bool,
    #[serde(
        rename = "CreateTime",
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    pub(crate) create_time: Option<Timestamp>,
    #[serde(
        rename = "ModifyTime",
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    pub(crate) modify_time: Option<Timestamp>,
}
