use std::collections::BTreeMap;

use futures_util::{StreamExt, TryStreamExt, stream};

use crate::ItemRevision;
use crate::api::{
    ItemLatestKey, MoveToFolderItem, MoveToFolderRequest, MoveToShareItem, MoveToShareRequest,
    MovedItem,
};
use crate::client::{LARGE_BATCH, PREFIX, PassClient};
use crate::crypto::{EncryptionKey, EncryptionTag, ItemKey, encode_base64};
use crate::error::{Error, Result};
use crate::identifier::{FolderId, ItemId, ShareId};
use crate::pass_item::PassItem;

const KEY_FETCH_CONCURRENCY: usize = 8;

impl PassClient {
    pub async fn move_items(
        &self,
        items: &[PassItem],
        destination: &ShareId,
        folder: Option<&FolderId>,
    ) -> Result<Vec<PassItem>> {
        let target = self.destination_key(destination, folder).await?;

        let mut by_share: BTreeMap<&ShareId, Vec<&PassItem>> = BTreeMap::new();
        for item in items {
            by_share.entry(item.share_id()).or_default().push(item);
        }

        let mut moved = Vec::with_capacity(items.len());
        for (share, share_items) in by_share {
            for batch in share_items.chunks(LARGE_BATCH) {
                let requests = self
                    .reencrypt_item_keys(share, batch.iter().copied(), &target)
                    .await?
                    .into_iter()
                    .map(|(item_id, keys)| MoveToShareItem {
                        item_id,
                        folder_id: folder.cloned(),
                        keys,
                    })
                    .collect();

                let envelope: Vec<ItemRevision> = self
                    .api()
                    .put(format!("{PREFIX}/share/{share}/item/share"))
                    .json(&MoveToShareRequest {
                        share_id: destination.clone(),
                        items: requests,
                    })
                    .send_field_or_default("Items")
                    .await?;
                moved.extend(envelope);
            }
        }

        self.decrypt_items(destination, moved).await
    }

    pub async fn move_items_to_folder(
        &self,
        share: &ShareId,
        items: &[PassItem],
        folder: Option<&FolderId>,
    ) -> Result<Vec<MovedItem>> {
        let target = self.destination_key(share, folder).await?;
        let mut moved = Vec::with_capacity(items.len());

        for batch in items.chunks(LARGE_BATCH) {
            let requests = self
                .reencrypt_item_keys(share, batch, &target)
                .await?
                .into_iter()
                .map(|(item_id, keys)| MoveToFolderItem { item_id, keys })
                .collect();

            let envelope: Vec<MovedItem> = self
                .api()
                .put(format!("{PREFIX}/share/{share}/item/folder"))
                .json(&MoveToFolderRequest {
                    folder_id: folder.cloned(),
                    items: requests,
                })
                .send_field_or_default("Items")
                .await?;
            moved.extend(envelope);
        }

        Ok(moved)
    }

    async fn destination_key(
        &self,
        share: &ShareId,
        folder: Option<&FolderId>,
    ) -> Result<EncryptionKey> {
        Ok(match folder {
            Some(folder) => self.folder_key(share, folder).await?.key().clone(),
            None => self.latest_share_key(share).await?.key().clone(),
        })
    }

    async fn reencrypt_item_keys<'a>(
        &self,
        share: &ShareId,
        items: impl IntoIterator<Item = &'a PassItem>,
        target: &EncryptionKey,
    ) -> Result<Vec<(ItemId, Vec<ItemLatestKey>)>> {
        stream::iter(items)
            .map(|item| async move {
                let keys = self.revision_item_keys(share, item.response()).await?;
                Ok::<_, Error>((item.id().clone(), encrypt_item_keys(target, &keys)?))
            })
            .buffered(KEY_FETCH_CONCURRENCY)
            .try_collect()
            .await
    }
}

fn encrypt_item_keys(target: &EncryptionKey, keys: &[ItemKey]) -> Result<Vec<ItemLatestKey>> {
    keys.iter()
        .map(|key| {
            let encrypted = target.seal(key.key().as_bytes(), &EncryptionTag::ItemKey)?;
            Ok(ItemLatestKey::new(key.rotation(), encode_base64(encrypted)))
        })
        .collect()
}
