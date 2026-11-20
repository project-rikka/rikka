use rikka_proton_authenticator::{AuthenticatorClient, LocalEntry, SyncOutcome};

use crate::error::Result;
use crate::stored::StoredCode;

pub async fn sync(client: &AuthenticatorClient, codes: &[StoredCode]) -> Result<SyncOutcome> {
    let local: Vec<LocalEntry> = codes.iter().map(StoredCode::local).collect();
    Ok(client.sync(&local).await?)
}

pub async fn recall(client: &AuthenticatorClient) -> Result<Vec<StoredCode>> {
    let Some(keys) = client.cached_keys().await? else {
        return Ok(Vec::new());
    };
    let entries = client.cached_entries(&keys).await.unwrap_or_default();
    Ok(entries.iter().map(StoredCode::from_remote).collect())
}
