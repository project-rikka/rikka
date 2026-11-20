use base64::prelude::{BASE64_URL_SAFE_NO_PAD, Engine as _};

use crate::content::Item;
use crate::crypto::key::EncryptionKey;
use crate::crypto::tag::EncryptionTag;
use crate::crypto::{decode_base64, encode_base64};
use crate::error::{Error, Result};

pub(crate) struct LinkKeys {
    pub(crate) link_key: EncryptionKey,
    pub(crate) encrypted_item_key: String,
    pub(crate) encrypted_link_key: String,
}

pub(crate) fn generate_link_keys(item_key: &EncryptionKey) -> Result<LinkKeys> {
    let link_key = EncryptionKey::generate();
    let encrypted_item_key = link_key.seal(item_key.as_bytes(), &EncryptionTag::ItemKey)?;
    let encrypted_link_key = item_key.seal(link_key.as_bytes(), &EncryptionTag::LinkKey)?;

    Ok(LinkKeys {
        link_key,
        encrypted_item_key: encode_base64(encrypted_item_key),
        encrypted_link_key: encode_base64(encrypted_link_key),
    })
}

pub(crate) fn decrypt_link_key(key: &EncryptionKey, encrypted: &str) -> Result<EncryptionKey> {
    let sealed = decode_base64("link key", encrypted)?;

    EncryptionKey::from_bytes(key.open(sealed, &EncryptionTag::LinkKey)?)
}

pub(crate) fn link_url(url: &str, link_key: &EncryptionKey) -> String {
    format!(
        "{url}#{}",
        BASE64_URL_SAFE_NO_PAD.encode(link_key.as_bytes())
    )
}

pub(crate) fn parse_link_url(link: &str) -> Result<(String, EncryptionKey)> {
    let malformed = || Error::MalformedLink(link.to_owned());

    let (location, fragment) = link.split_once('#').ok_or_else(malformed)?;
    let token = location
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|token| !token.is_empty() && !token.contains(':'))
        .ok_or_else(malformed)?;

    let raw = BASE64_URL_SAFE_NO_PAD
        .decode(fragment.trim().trim_end_matches('='))
        .map_err(|_| malformed())?;
    let key = EncryptionKey::from_bytes(raw).map_err(|_| malformed())?;

    Ok((token.to_owned(), key))
}

pub(crate) fn decrypt_item_key(link_key: &EncryptionKey, encrypted: &str) -> Result<EncryptionKey> {
    let sealed = decode_base64("secure link item key", encrypted)?;

    EncryptionKey::from_bytes(link_key.open(sealed, &EncryptionTag::ItemKey)?)
}

pub(crate) fn decrypt_item_content(item_key: &EncryptionKey, content: &str) -> Result<Item> {
    let sealed = decode_base64("secure link content", content)?;

    Item::decode(&item_key.open(sealed, &EncryptionTag::ItemContent)?)
}
