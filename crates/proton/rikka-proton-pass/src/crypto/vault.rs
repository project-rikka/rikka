use rikka_proton_account::{AddressId, UserKeys};
use rikka_proton_pgp::Pgp;

use crate::api::{CreateVaultRequest, UpdateVaultRequest};
use crate::content::Vault;
use crate::crypto::key::EncryptionKey;
use crate::crypto::rotated::ShareKey;
use crate::crypto::tag::EncryptionTag;
use crate::crypto::{decode_base64, encode_base64};
use crate::error::Result;

pub(crate) const CONTENT_FORMAT_VERSION: i32 = 1;

pub(crate) fn create_request(
    pgp: &Pgp,
    keys: &UserKeys,
    address: AddressId,
    vault: &Vault,
) -> Result<(CreateVaultRequest, EncryptionKey)> {
    let vault_key = EncryptionKey::generate();
    let content = vault_key.seal(vault.encode(), &EncryptionTag::VaultContent)?;

    let request = CreateVaultRequest {
        address_id: address,
        content: encode_base64(content),
        content_format_version: CONTENT_FORMAT_VERSION,
        encrypted_vault_key: vault_key.wrap(pgp, keys)?,
    };

    Ok((request, vault_key))
}

pub(crate) fn update_request(key: &ShareKey, vault: &Vault) -> Result<UpdateVaultRequest> {
    let content = key
        .key()
        .seal(vault.encode(), &EncryptionTag::VaultContent)?;

    Ok(UpdateVaultRequest {
        content: encode_base64(content),
        content_format_version: CONTENT_FORMAT_VERSION,
        key_rotation: key.rotation(),
    })
}

pub(crate) fn decrypt_content(key: &ShareKey, content: &str) -> Result<Vault> {
    let sealed = decode_base64("vault content", content)?;
    let decrypted = key.key().open(sealed, &EncryptionTag::VaultContent)?;

    Vault::decode(&decrypted)
}
