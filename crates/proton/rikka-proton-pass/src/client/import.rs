use std::collections::HashMap;

use crate::api::ItemPage;
use crate::api::{ImportItemRequest, ImportItemsRequest};
use crate::client::{LARGE_BATCH, PREFIX, PassClient};
use crate::crypto::RotatedKey;
use crate::error::{Error, Result};
use crate::identifier::{FolderId, ShareId};
use crate::import_item::ImportItem;
use crate::pass_item::PassItem;

impl PassClient {
    pub async fn import_items(
        &self,
        share: &ShareId,
        items: impl IntoIterator<Item = ImportItem>,
    ) -> Result<Vec<PassItem>> {
        let items: Vec<ImportItem> = items.into_iter().collect();
        if items.is_empty() {
            return Ok(Vec::new());
        }

        let root = self.latest_share_key(share).await?;
        let mut folders: Vec<&FolderId> = items
            .iter()
            .filter_map(|import| import.item.folder.as_ref())
            .collect();
        folders.sort_unstable();
        folders.dedup();

        let folder_keys = if folders.is_empty() {
            HashMap::new()
        } else {
            self.folder_keys_for(share, &folders).await?
        };
        if let Some(missing) = folders
            .into_iter()
            .find(|folder| !folder_keys.contains_key(*folder))
        {
            return Err(Error::MissingFolder {
                share: share.clone(),
                folder: missing.clone(),
            });
        }

        let folder_keys: HashMap<FolderId, RotatedKey> = folder_keys
            .into_iter()
            .map(|(folder, key)| (folder, key.into()))
            .collect();
        let root = RotatedKey::from(root);
        let requests = rikka_proton_api::parallel_map(items, move |import| {
            let parent = match &import.item.folder {
                Some(folder) => &folder_keys[folder],
                None => &root,
            };
            Ok(ImportItemRequest {
                item: crate::crypto::item::create_request(parent, &import.item)?,
                alias_email: import.alias_email,
                trashed: import.trashed,
                create_time: import.created_at,
                modify_time: import.modified_at,
            })
        })
        .await
        .into_iter()
        .collect::<Result<Vec<ImportItemRequest>>>()?;

        let mut revisions = Vec::with_capacity(requests.len());
        for batch in requests.chunks(LARGE_BATCH) {
            let envelope: ItemPage = self
                .api()
                .post(format!("{PREFIX}/share/{share}/item/import/batch"))
                .json(&ImportItemsRequest {
                    items: batch.to_vec(),
                })
                .send_field("Revisions")
                .await?;
            revisions.extend(envelope.revisions);
        }

        self.decrypt_items(share, revisions).await
    }
}
