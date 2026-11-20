use rikka_proton_account::AddressId;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CreateVaultRequest {
    #[serde(rename = "AddressID")]
    pub(crate) address_id: AddressId,
    #[serde(rename = "Content")]
    pub(crate) content: String,
    #[serde(rename = "ContentFormatVersion")]
    pub(crate) content_format_version: i32,
    #[serde(rename = "EncryptedVaultKey")]
    pub(crate) encrypted_vault_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UpdateVaultRequest {
    #[serde(rename = "Content")]
    pub(crate) content: String,
    #[serde(rename = "ContentFormatVersion")]
    pub(crate) content_format_version: i32,
    #[serde(rename = "KeyRotation")]
    pub(crate) key_rotation: i64,
}
