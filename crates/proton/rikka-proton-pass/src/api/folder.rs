use serde::{Deserialize, Serialize};

use crate::identifier::{FolderId, VaultId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FolderResponse {
    #[serde(rename = "FolderID")]
    folder_id: FolderId,
    #[serde(rename = "VaultID", default)]
    vault_id: VaultId,
    #[serde(
        rename = "ParentFolderID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    parent_folder_id: Option<FolderId>,
    #[serde(rename = "KeyRotation", default)]
    key_rotation: i64,
    #[serde(rename = "FolderKey", default)]
    folder_key: String,
    #[serde(rename = "ContentFormatVersion", default)]
    content_format_version: i32,
    #[serde(rename = "Content", default)]
    content: String,
}

impl FolderResponse {
    #[must_use]
    pub fn folder_id(&self) -> &FolderId {
        &self.folder_id
    }

    #[must_use]
    pub fn vault_id(&self) -> &VaultId {
        &self.vault_id
    }

    #[must_use]
    pub fn parent_folder_id(&self) -> Option<&FolderId> {
        self.parent_folder_id.as_ref()
    }

    #[must_use]
    pub fn key_rotation(&self) -> i64 {
        self.key_rotation
    }

    #[must_use]
    pub fn folder_key(&self) -> &str {
        &self.folder_key
    }

    #[must_use]
    pub fn content_format_version(&self) -> i32 {
        self.content_format_version
    }

    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateFolderRequest {
    #[serde(
        rename = "ParentFolderID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) parent_folder_id: Option<FolderId>,
    #[serde(rename = "ContentFormatVersion")]
    pub(crate) content_format_version: i32,
    #[serde(rename = "Content")]
    pub(crate) content: String,
    #[serde(rename = "KeyRotation")]
    pub(crate) key_rotation: i64,
    #[serde(rename = "FolderKey")]
    pub(crate) folder_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FolderContentRequest {
    #[serde(rename = "KeyRotation")]
    pub(crate) key_rotation: i64,
    #[serde(rename = "ContentFormatVersion")]
    pub(crate) content_format_version: i32,
    #[serde(rename = "Content")]
    pub(crate) content: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UpdateFolderRequest {
    #[serde(rename = "Content")]
    pub(crate) content: FolderContentRequest,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FolderKeyRequest {
    #[serde(rename = "KeyRotation")]
    pub(crate) key_rotation: i64,
    #[serde(rename = "FolderKey")]
    pub(crate) folder_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MoveFolderRequest {
    #[serde(
        rename = "ParentFolderID",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) parent_folder_id: Option<FolderId>,
    #[serde(rename = "FolderKeys")]
    pub(crate) folder_keys: Vec<FolderKeyRequest>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DeleteFoldersRequest {
    #[serde(rename = "FolderIDs")]
    pub(crate) folder_ids: Vec<FolderId>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct FolderPage {
    #[serde(rename = "Folders", default)]
    pub(crate) folders: Vec<FolderResponse>,
    #[serde(rename = "LastToken", default)]
    pub(crate) last_token: Option<String>,
}
