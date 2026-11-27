use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;

pub const KEY_RETRIEVAL_PAGE: &str = "https://accounts.google.com/encryption/unlock/desktop";
pub const KEY_RETRIEVAL_SITE: &str = "https://accounts.google.com/encryption/unlock/*";
pub const KEY_RETRIEVAL_SCRIPT: &str = "google-passwords/vault_keys.js";
pub const KEY_RETRIEVAL_MESSAGE: &str = "rikkaGoogleVaultKeys";

#[derive(Deserialize)]
struct Posted {
    gaia: String,
    keys: Vec<String>,
}

#[must_use]
pub fn posted_vault_keys(body: &serde_json::Value, account_id: &str) -> Option<Vec<Vec<u8>>> {
    let posted = Posted::deserialize(body).ok()?;
    if posted.gaia != account_id {
        return None;
    }
    let keys: Vec<Vec<u8>> = posted
        .keys
        .iter()
        .filter_map(|key| STANDARD.decode(key).ok())
        .filter(|key| !key.is_empty())
        .collect();
    (!keys.is_empty()).then_some(keys)
}
