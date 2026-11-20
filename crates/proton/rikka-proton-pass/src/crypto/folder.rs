use crate::api::{
    CreateFolderRequest, FolderContentRequest, FolderKeyRequest, FolderResponse,
    UpdateFolderRequest,
};
use crate::content::Folder;
use crate::crypto::key::EncryptionKey;
use crate::crypto::rotated::{FolderKey, RotatedKey};
use crate::crypto::tag::EncryptionTag;
use crate::crypto::{decode_base64, encode_base64};
use crate::error::Result;
use crate::identifier::FolderId;

pub(crate) const CONTENT_FORMAT_VERSION: i32 = 1;

pub(crate) fn create_request(
    parent: &RotatedKey,
    parent_folder: Option<FolderId>,
    name: impl Into<String>,
) -> Result<(CreateFolderRequest, EncryptionKey)> {
    let folder = Folder { name: name.into() };
    let folder_key = EncryptionKey::generate();

    let content = folder_key.seal(folder.encode(), &EncryptionTag::FolderContent)?;
    let encrypted_key = parent
        .key
        .seal(folder_key.as_bytes(), &EncryptionTag::FolderKey)?;

    let request = CreateFolderRequest {
        parent_folder_id: parent_folder,
        content_format_version: CONTENT_FORMAT_VERSION,
        content: encode_base64(content),
        key_rotation: parent.rotation,
        folder_key: encode_base64(encrypted_key),
    };

    Ok((request, folder_key))
}

pub(crate) fn update_request(
    key: &FolderKey,
    name: impl Into<String>,
) -> Result<UpdateFolderRequest> {
    let folder = Folder { name: name.into() };
    let content = key
        .key()
        .seal(folder.encode(), &EncryptionTag::FolderContent)?;

    Ok(UpdateFolderRequest {
        content: FolderContentRequest {
            key_rotation: key.rotation(),
            content_format_version: CONTENT_FORMAT_VERSION,
            content: encode_base64(content),
        },
    })
}

pub(crate) fn reencrypt_key(key: &FolderKey, parent: &RotatedKey) -> Result<FolderKeyRequest> {
    let encrypted = parent
        .key
        .seal(key.key().as_bytes(), &EncryptionTag::FolderKey)?;

    Ok(FolderKeyRequest {
        key_rotation: parent.rotation,
        folder_key: encode_base64(encrypted),
    })
}

pub(crate) fn decrypt_key(parent: &EncryptionKey, folder: &FolderResponse) -> Result<FolderKey> {
    let sealed = decode_base64("folder key", folder.folder_key())?;
    let decrypted = parent.open(sealed, &EncryptionTag::FolderKey)?;

    Ok(FolderKey::new(
        folder.key_rotation(),
        EncryptionKey::from_bytes(decrypted)?,
    ))
}

pub(crate) fn decrypt_content(key: &FolderKey, folder: &FolderResponse) -> Result<Folder> {
    let sealed = decode_base64("folder content", folder.content())?;
    let decrypted = key.key().open(sealed, &EncryptionTag::FolderContent)?;

    Folder::decode(&decrypted)
}
