use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::identifier::{ItemId, SecureLinkId, ShareId};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateSecureLinkRequest {
    #[serde(rename = "Revision")]
    pub(crate) revision: i64,
    #[serde(rename = "ExpirationTime")]
    pub(crate) expiration_time: u64,
    #[serde(rename = "MaxReadCount", skip_serializing_if = "Option::is_none")]
    pub(crate) max_read_count: Option<u32>,
    #[serde(rename = "EncryptedItemKey")]
    pub(crate) encrypted_item_key: String,
    #[serde(rename = "EncryptedLinkKey")]
    pub(crate) encrypted_link_key: String,
    #[serde(rename = "LinkKeyShareKeyRotation")]
    pub(crate) link_key_share_key_rotation: i64,
    #[serde(rename = "LinkKeyEncryptedWithItemKey")]
    pub(crate) link_key_encrypted_with_item_key: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreatedSecureLink {
    #[serde(rename = "Url")]
    pub(crate) url: String,
    #[serde(rename = "PublicLinkID")]
    pub(crate) id: SecureLinkId,
    #[serde(
        rename = "ExpirationTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    pub(crate) expiration_time: Timestamp,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SecureLinkResponse {
    #[serde(rename = "LinkID")]
    pub(crate) id: SecureLinkId,
    #[serde(rename = "ReadCount", default)]
    pub(crate) read_count: i64,
    #[serde(rename = "MaxReadCount", default)]
    pub(crate) max_read_count: Option<i64>,
    #[serde(
        rename = "ExpirationTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    pub(crate) expiration_time: Timestamp,
    #[serde(rename = "ShareID")]
    pub(crate) share_id: ShareId,
    #[serde(rename = "ItemID")]
    pub(crate) item_id: ItemId,
    #[serde(rename = "LinkURL", default)]
    pub(crate) url: String,
    #[serde(rename = "EncryptedLinkKey", default)]
    pub(crate) encrypted_link_key: String,
    #[serde(rename = "LinkKeyShareKeyRotation", default)]
    pub(crate) link_key_share_key_rotation: i64,
    #[serde(rename = "Active", default)]
    pub(crate) active: bool,
    #[serde(rename = "LinkKeyEncryptedWithItemKey", default)]
    pub(crate) link_key_encrypted_with_item_key: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SealedLinkContent {
    #[serde(rename = "Contents")]
    pub(crate) contents: String,
    #[serde(rename = "ItemKey")]
    pub(crate) item_key: String,
    #[serde(rename = "FilesToken", default)]
    pub(crate) files_token: Option<String>,
    #[serde(rename = "ContentFormatVersion", default)]
    pub(crate) content_format_version: i32,
    #[serde(rename = "ReadCount", default)]
    pub(crate) read_count: i64,
    #[serde(rename = "MaxReadCount", default)]
    pub(crate) max_read_count: Option<i64>,
    #[serde(
        rename = "ExpirationTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    pub(crate) expiration_time: Timestamp,
}
