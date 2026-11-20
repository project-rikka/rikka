use crate::api::{CreateItemRequest, ItemRevision, UpdateItemRequest};
use crate::content::Item;
use crate::crypto::key::EncryptionKey;
use crate::crypto::rotated::{ItemKey, RotatedKey};
use crate::crypto::tag::EncryptionTag;
use crate::crypto::{decode_base64, encode_base64};
use crate::error::Result;
use crate::new_item::NewItem;

pub(crate) const CONTENT_FORMAT_VERSION: i32 = 8;
pub(crate) const CONTENT_FORMAT_VERSION_WITHOUT_URL_MATCHING: i32 = 7;

pub(crate) fn content_format_version(url_matching: bool) -> i32 {
    if url_matching {
        CONTENT_FORMAT_VERSION
    } else {
        CONTENT_FORMAT_VERSION_WITHOUT_URL_MATCHING
    }
}

pub(crate) fn create_request(parent: &RotatedKey, item: &NewItem) -> Result<CreateItemRequest> {
    let item_key = EncryptionKey::generate();
    let content = item_key.seal(item.content.encode(), &EncryptionTag::ItemContent)?;
    let encrypted_key = parent
        .key
        .seal(item_key.as_bytes(), &EncryptionTag::ItemKey)?;

    Ok(CreateItemRequest {
        key_rotation: parent.rotation,
        content_format_version: content_format_version(item.url_matching),
        content: encode_base64(content),
        item_key: encode_base64(encrypted_key),
        folder_id: item.folder.clone(),
    })
}

pub(crate) fn update_request(
    key: &ItemKey,
    item: &Item,
    last_revision: i64,
    url_matching: bool,
) -> Result<UpdateItemRequest> {
    let content = key.key().seal(item.encode(), &EncryptionTag::ItemContent)?;

    Ok(UpdateItemRequest {
        key_rotation: key.rotation(),
        last_revision,
        content_format_version: content_format_version(url_matching),
        content: encode_base64(content),
    })
}

pub(crate) fn decrypt_key(parent: &EncryptionKey, rotation: i64, key: &str) -> Result<ItemKey> {
    let sealed = decode_base64("item key", key)?;
    let decrypted = parent.open(sealed, &EncryptionTag::ItemKey)?;

    Ok(ItemKey::new(
        rotation,
        EncryptionKey::from_bytes(decrypted)?,
    ))
}

pub(crate) fn decrypt_content(key: &ItemKey, revision: &ItemRevision) -> Result<Item> {
    let sealed = decode_base64("item content", revision.content())?;
    let decrypted = key.key().open(sealed, &EncryptionTag::ItemContent)?;

    Item::decode(&decrypted)
}
