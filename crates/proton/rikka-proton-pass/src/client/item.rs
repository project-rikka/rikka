use jiff::Timestamp;
use std::collections::HashMap;

use rikka_proton_api::{Page, Paging};

use crate::ItemLatestKey;
use crate::api::{
    ItemFlagsRequest, ItemPage, ItemRevision, ItemTarget, ItemTargets, LastUsedRequest, TrashedItem,
};
use crate::client::{PAGE_SIZE, PREFIX, PassClient};
use crate::content::Item;
use crate::crypto::{EncryptionKey, FolderKey, ItemKey, RotatedKey, ShareKeys};
use crate::error::{Error, Result};
use crate::identifier::{FolderId, ItemId, ShareId};
use crate::new_item::NewItem;
use crate::pass_item::PassItem;

impl PassClient {
    pub async fn items(&self, share: &ShareId) -> Result<Vec<PassItem>> {
        let revisions = self.share_revisions(share).await?;
        self.cache_items(share, &revisions, None);

        self.decrypt_items(share, revisions).await
    }

    pub(crate) async fn share_revisions(&self, share: &ShareId) -> Result<Vec<ItemRevision>> {
        self.paged_items(format!("{PREFIX}/share/{share}/item"), "Items")
            .await
    }

    pub async fn item(&self, share: &ShareId, item: &ItemId) -> Result<PassItem> {
        let revision = self.item_revision(share, item).await?;

        self.decrypt_item(share, revision).await
    }

    pub async fn item_revisions(&self, share: &ShareId, item: &ItemId) -> Result<Vec<PassItem>> {
        let revisions = self
            .paged_items(
                format!("{PREFIX}/share/{share}/item/{item}/revision"),
                "Revisions",
            )
            .await?;

        self.decrypt_items(share, revisions).await
    }

    pub async fn create_item(&self, share: &ShareId, item: NewItem) -> Result<PassItem> {
        let parent: RotatedKey = match &item.folder {
            Some(folder) => self.folder_key(share, folder).await?.into(),
            None => self.latest_share_key(share).await?.into(),
        };
        let request = crate::crypto::item::create_request(&parent, &item)?;

        let envelope: ItemRevision = self
            .api()
            .post(format!("{PREFIX}/share/{share}/item"))
            .json(&request)
            .send_field("Item")
            .await?;

        decrypt_revision(share, &parent.key, envelope)
    }

    pub async fn update_item(&self, item: &PassItem, content: &Item) -> Result<PassItem> {
        let share = item.share_id();
        let id = item.id();
        let url_matching = item.response().content_format_version()
            != crate::crypto::item::CONTENT_FORMAT_VERSION_WITHOUT_URL_MATCHING;

        let request = crate::crypto::item::update_request(
            item.key(),
            content,
            item.revision(),
            url_matching,
        )?;

        let envelope: ItemRevision = self
            .api()
            .put(format!("{PREFIX}/share/{share}/item/{id}"))
            .json(&request)
            .send_field("Item")
            .await?;

        Ok(PassItem::new(
            share.clone(),
            envelope,
            content.clone(),
            item.key().clone(),
        ))
    }

    pub async fn trash_items(
        &self,
        share: &ShareId,
        items: impl IntoIterator<Item = ItemTarget>,
    ) -> Result<Vec<TrashedItem>> {
        self.post_trash_action(share, "trash", items).await
    }

    pub async fn untrash_items(
        &self,
        share: &ShareId,
        items: impl IntoIterator<Item = ItemTarget>,
    ) -> Result<Vec<TrashedItem>> {
        self.post_trash_action(share, "untrash", items).await
    }

    pub async fn delete_items(
        &self,
        share: &ShareId,
        items: impl IntoIterator<Item = ItemTarget>,
    ) -> Result<()> {
        let body = ItemTargets {
            items: items.into_iter().collect(),
        };

        self.api()
            .delete(format!("{PREFIX}/share/{share}/item"))
            .json(&body)
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn item_key(&self, share: &ShareId, item: &ItemId) -> Result<ItemKey> {
        let envelope: ItemLatestKey = self
            .api()
            .get(format!("{PREFIX}/share/{share}/item/{item}/key/latest"))
            .send_field("Key")
            .await?;

        let revision = self.item_revision(share, item).await?;
        let parent = self.parent_key_for(share, &revision).await?;

        crate::crypto::item::decrypt_key(&parent, envelope.rotation(), envelope.key())
    }

    pub async fn item_keys(&self, share: &ShareId, item: &ItemId) -> Result<Vec<ItemKey>> {
        let revision = self.item_revision(share, item).await?;

        self.revision_item_keys(share, &revision).await
    }

    pub(crate) async fn revision_item_keys(
        &self,
        share: &ShareId,
        revision: &ItemRevision,
    ) -> Result<Vec<ItemKey>> {
        if revision.item_key().is_none() {
            let keys = self.share_keys(share).await?;
            return Ok(keys
                .keys()
                .iter()
                .map(|key| ItemKey::new(key.rotation(), key.key().clone()))
                .collect());
        }

        let item = revision.item_id();
        let sealed: Option<Vec<ItemLatestKey>> = self
            .api()
            .get(format!("{PREFIX}/share/{share}/item/{item}/key"))
            .send_field_at(&["Keys", "Keys"])
            .await?;
        let sealed = sealed.unwrap_or_default();

        if let Some(folder) = revision.folder_id() {
            let parent = self.folder_key(share, folder).await?;
            return sealed
                .iter()
                .map(|key| {
                    crate::crypto::item::decrypt_key(parent.key(), key.rotation(), key.key())
                })
                .collect();
        }

        let rotations: Vec<i64> = sealed.iter().map(|key| key.rotation()).collect();
        let keys = self.share_keys_for_rotations(share, &rotations).await?;

        sealed
            .iter()
            .map(|key| {
                let parent = keys
                    .get(key.rotation())
                    .ok_or_else(|| Error::MissingShareKey {
                        share: share.clone(),
                        rotation: key.rotation(),
                    })?;
                crate::crypto::item::decrypt_key(parent.key(), key.rotation(), key.key())
            })
            .collect()
    }

    pub async fn pin_item(&self, share: &ShareId, item: &ItemId) -> Result<PassItem> {
        let envelope: ItemRevision = self
            .api()
            .post(format!("{PREFIX}/share/{share}/item/{item}/pin"))
            .send_field("Item")
            .await?;

        self.decrypt_item(share, envelope).await
    }

    pub async fn unpin_item(&self, share: &ShareId, item: &ItemId) -> Result<PassItem> {
        let envelope: ItemRevision = self
            .api()
            .delete(format!("{PREFIX}/share/{share}/item/{item}/pin"))
            .send_field("Item")
            .await?;

        self.decrypt_item(share, envelope).await
    }

    pub async fn set_item_flags(
        &self,
        share: &ShareId,
        item: &ItemId,
        flags: ItemFlagsRequest,
    ) -> Result<PassItem> {
        let envelope: ItemRevision = self
            .api()
            .put(format!("{PREFIX}/share/{share}/item/{item}/flags"))
            .json(&flags)
            .send_field("Item")
            .await?;

        self.decrypt_item(share, envelope).await
    }

    pub async fn mark_item_used(
        &self,
        share: &ShareId,
        item: &ItemId,
        at: Timestamp,
    ) -> Result<ItemRevision> {
        let envelope: ItemRevision = self
            .api()
            .put(format!("{PREFIX}/share/{share}/item/{item}/lastuse"))
            .json(&LastUsedRequest { last_use_time: at })
            .send_field("Revision")
            .await?;

        Ok(envelope)
    }

    pub(crate) async fn decrypt_item(
        &self,
        share: &ShareId,
        revision: ItemRevision,
    ) -> Result<PassItem> {
        let parent = self.parent_key_for(share, &revision).await?;

        decrypt_revision(share, &parent, revision)
    }

    pub(crate) async fn decrypt_items(
        &self,
        share: &ShareId,
        revisions: Vec<ItemRevision>,
    ) -> Result<Vec<PassItem>> {
        if revisions.is_empty() {
            return Ok(Vec::new());
        }

        let parent_keys = self.parent_keys_for_all(share, &revisions).await?;
        let encrypted: Vec<(EncryptionKey, ItemRevision)> =
            parent_keys.into_iter().zip(revisions).collect();
        let share = share.clone();

        rikka_proton_api::parallel_map(encrypted, move |(parent, revision)| {
            decrypt_revision(&share, &parent, revision)
        })
        .await
        .into_iter()
        .collect()
    }

    pub(crate) async fn item_revision(
        &self,
        share: &ShareId,
        item: &ItemId,
    ) -> Result<ItemRevision> {
        let envelope: ItemRevision = self
            .api()
            .get(format!("{PREFIX}/share/{share}/item/{item}"))
            .send_field("Item")
            .await?;

        Ok(envelope)
    }

    pub(crate) async fn parent_key_for(
        &self,
        share: &ShareId,
        revision: &ItemRevision,
    ) -> Result<EncryptionKey> {
        let (keys, folders) = self
            .parent_keys(share, std::slice::from_ref(revision))
            .await?;

        parent_key(share, &keys, &folders, revision)
    }

    async fn parent_keys_for_all(
        &self,
        share: &ShareId,
        revisions: &[ItemRevision],
    ) -> Result<Vec<EncryptionKey>> {
        let (keys, folders) = self.parent_keys(share, revisions).await?;

        revisions
            .iter()
            .map(|revision| parent_key(share, &keys, &folders, revision))
            .collect()
    }

    async fn parent_keys(
        &self,
        share: &ShareId,
        revisions: &[ItemRevision],
    ) -> Result<(ShareKeys, HashMap<FolderId, FolderKey>)> {
        let mut rotations: Vec<i64> = revisions
            .iter()
            .filter(|revision| revision.folder_id().is_none())
            .map(ItemRevision::key_rotation)
            .collect();
        rotations.sort_unstable();
        rotations.dedup();

        let mut folders: Vec<&FolderId> = revisions
            .iter()
            .filter_map(ItemRevision::folder_id)
            .collect();
        folders.sort_unstable();
        folders.dedup();

        let keys = if rotations.is_empty() {
            ShareKeys::default()
        } else {
            self.share_keys_for_rotations(share, &rotations).await?
        };
        let folder_keys = if folders.is_empty() {
            HashMap::new()
        } else {
            self.folder_keys_for(share, &folders).await?
        };

        Ok((keys, folder_keys))
    }

    async fn post_trash_action(
        &self,
        share: &ShareId,
        action: &str,
        items: impl IntoIterator<Item = ItemTarget>,
    ) -> Result<Vec<TrashedItem>> {
        let body = ItemTargets {
            items: items.into_iter().collect(),
        };

        let envelope: Vec<TrashedItem> = self
            .api()
            .post(format!("{PREFIX}/share/{share}/item/{action}"))
            .json(&body)
            .send_field_or_default("Items")
            .await?;

        Ok(envelope)
    }

    async fn paged_items(&self, path: String, field: &'static str) -> Result<Vec<ItemRevision>> {
        Ok(self
            .api()
            .get(path)
            .pages(Paging::since().page_size(PAGE_SIZE))
            .field(field)
            .collect(|page: ItemPage| Page::new(page.revisions).next(page.last_token))
            .await?)
    }
}

pub(crate) fn decrypt_revision(
    share: &ShareId,
    parent: &EncryptionKey,
    revision: ItemRevision,
) -> Result<PassItem> {
    let key = match revision.item_key() {
        Some(sealed) => crate::crypto::item::decrypt_key(parent, revision.key_rotation(), sealed)?,
        None => ItemKey::new(revision.key_rotation(), parent.clone()),
    };

    let content = crate::crypto::item::decrypt_content(&key, &revision)?;
    Ok(PassItem::new(share.clone(), revision, content, key))
}

fn parent_key(
    share: &ShareId,
    keys: &ShareKeys,
    folders: &HashMap<FolderId, FolderKey>,
    revision: &ItemRevision,
) -> Result<EncryptionKey> {
    match revision.folder_id() {
        Some(folder) => folders
            .get(folder)
            .map(|key| key.key().clone())
            .ok_or_else(|| Error::MissingFolder {
                share: share.clone(),
                folder: folder.clone(),
            }),
        None => keys
            .get(revision.key_rotation())
            .map(|key| key.key().clone())
            .ok_or_else(|| Error::MissingShareKey {
                share: share.clone(),
                rotation: revision.key_rotation(),
            }),
    }
}
