use futures_util::future::join_all;
use rikka_proton_pass::{PassClient, ShareId};

use crate::decrypted_item::DecryptedItem;
use crate::error::Result;
use crate::store::Vault;

#[derive(Debug, Default)]
pub struct Recalled {
    pub vaults: Vec<Vault>,
    pub lists: Vec<(ShareId, Vec<DecryptedItem>)>,
}

pub async fn sync_items(client: &PassClient, share: &ShareId) -> Result<Vec<DecryptedItem>> {
    let items = client.sync_items(share).await?;
    Ok(DecryptedItem::all(items).await)
}

pub async fn load_cached(client: &PassClient) -> Result<Option<Recalled>> {
    let Some(shares) = client.cached_shares().await? else {
        return Ok(None);
    };
    let vaults: Vec<Vault> = shares.iter().filter_map(Vault::from_share).collect();
    let listing = vaults.iter().map(|vault| async move {
        let items = client.cached_items(&vault.share).await?;
        Ok::<_, crate::error::Error>(match items {
            Some(items) => Some((vault.share.clone(), DecryptedItem::all(items).await)),
            None => None,
        })
    });
    let mut lists = Vec::new();
    for listed in join_all(listing).await {
        lists.extend(listed?);
    }

    Ok(Some(Recalled { vaults, lists }))
}
