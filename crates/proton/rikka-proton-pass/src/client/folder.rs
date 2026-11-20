use std::collections::HashMap;

use rikka_proton_api::{Page, Paging};

use crate::api::{DeleteFoldersRequest, FolderPage, FolderResponse, MoveFolderRequest};
use crate::client::{PAGE_SIZE, PREFIX, PassClient, decrypt_folder_keys};
use crate::crypto::{EncryptionKey, FolderKey, RotatedKey, ShareKeys};
use crate::error::{Error, Result};
use crate::identifier::{FolderId, ShareId};
use crate::new_folder::NewFolder;
use crate::pass_folder::PassFolder;

impl PassClient {
    pub async fn folders(&self, share: &ShareId) -> Result<Vec<PassFolder>> {
        let responses = self.folder_responses(share).await?;
        let folder_keys = self.decrypt_all_folder_keys(share, &responses).await?;
        self.keyring
            .ring(share)
            .await
            .set_folders(folder_keys.clone());

        let mut folders = Vec::with_capacity(responses.len());
        for response in responses {
            let key = folder_keys
                .get(response.folder_id())
                .ok_or_else(|| Error::MissingFolder {
                    share: share.clone(),
                    folder: response.folder_id().clone(),
                })?
                .clone();

            folders.push(self.decrypt_folder(share, response, key)?);
        }

        Ok(folders)
    }

    pub async fn folder(&self, share: &ShareId, folder: &FolderId) -> Result<PassFolder> {
        let envelope: FolderResponse = self
            .api()
            .get(format!("{PREFIX}/share/{share}/folder/{folder}"))
            .send_field("Folder")
            .await?;

        let key = match envelope.parent_folder_id() {
            None => {
                let rotation = envelope.key_rotation();
                let keys = self.share_keys_for_rotations(share, &[rotation]).await?;
                let parent = share_key_by_rotation(share, &keys, rotation)?;
                crate::crypto::folder::decrypt_key(&parent, &envelope)?
            }
            Some(_) => self.folder_key(share, folder).await?,
        };

        self.decrypt_folder(share, envelope, key)
    }

    pub async fn create_folder(&self, share: &ShareId, folder: NewFolder) -> Result<PassFolder> {
        let NewFolder { name, parent } = folder;

        let parent_key: RotatedKey = match &parent {
            Some(parent) => self.folder_key(share, parent).await?.into(),
            None => self.latest_share_key(share).await?.into(),
        };

        let (request, folder_key) =
            crate::crypto::folder::create_request(&parent_key, parent, name)?;

        let envelope: FolderResponse = self
            .api()
            .post(format!("{PREFIX}/share/{share}/folder"))
            .json(&request)
            .send_field("Folder")
            .await?;

        let key = FolderKey::new(envelope.key_rotation(), folder_key);
        self.keyring
            .ring(share)
            .await
            .insert_folder(envelope.folder_id().clone(), key.clone());
        self.decrypt_folder(share, envelope, key)
    }

    pub async fn update_folder(
        &self,
        share: &ShareId,
        folder: &FolderId,
        name: impl Into<String>,
    ) -> Result<()> {
        let key = self.folder_key(share, folder).await?;
        let request = crate::crypto::folder::update_request(&key, name)?;

        self.api()
            .put(format!("{PREFIX}/share/{share}/folder/{folder}"))
            .json(&request)
            .send_unit()
            .await?;

        Ok(())
    }

    pub async fn delete_folders(
        &self,
        share: &ShareId,
        folders: impl IntoIterator<Item = FolderId>,
    ) -> Result<()> {
        let request = DeleteFoldersRequest {
            folder_ids: folders.into_iter().collect(),
        };

        self.api()
            .delete(format!("{PREFIX}/share/{share}/folder"))
            .json(&request)
            .send_unit()
            .await?;
        self.keyring
            .ring(share)
            .await
            .remove_folders(&request.folder_ids);

        Ok(())
    }

    pub async fn move_folder(
        &self,
        share: &ShareId,
        folder: &FolderId,
        parent: Option<&FolderId>,
    ) -> Result<()> {
        let folder_ids: Vec<&FolderId> = std::iter::once(folder).chain(parent).collect();
        let mut folders = self.folder_keys_for(share, &folder_ids).await?;
        let key = folders.remove(folder).ok_or_else(|| Error::MissingFolder {
            share: share.clone(),
            folder: folder.clone(),
        })?;

        let parent_key: RotatedKey = match parent {
            Some(parent) => folders
                .remove(parent)
                .ok_or_else(|| Error::MissingFolder {
                    share: share.clone(),
                    folder: parent.clone(),
                })?
                .into(),
            None => self.latest_share_key(share).await?.into(),
        };

        let key_request = crate::crypto::folder::reencrypt_key(&key, &parent_key)?;
        let request = MoveFolderRequest {
            parent_folder_id: parent.cloned(),
            folder_keys: vec![key_request],
        };

        self.api()
            .put(format!("{PREFIX}/share/{share}/folder/{folder}/move"))
            .json(&request)
            .send_unit()
            .await?;

        Ok(())
    }

    pub(crate) async fn folder_key(&self, share: &ShareId, folder: &FolderId) -> Result<FolderKey> {
        self.folder_keys_for(share, &[folder])
            .await?
            .remove(folder)
            .ok_or_else(|| Error::MissingFolder {
                share: share.clone(),
                folder: folder.clone(),
            })
    }

    pub(crate) async fn folder_keys_for(
        &self,
        share: &ShareId,
        folder_ids: &[&FolderId],
    ) -> Result<HashMap<FolderId, FolderKey>> {
        if let Some(cached) = self
            .keyring
            .ring(share)
            .await
            .folders_containing(folder_ids)
        {
            return Ok(cached.clone());
        }

        let responses = self.folder_responses(share).await?;
        let folder_keys = self.decrypt_all_folder_keys(share, &responses).await?;
        self.keyring
            .ring(share)
            .await
            .set_folders(folder_keys.clone());

        Ok(folder_keys)
    }

    async fn decrypt_all_folder_keys(
        &self,
        share: &ShareId,
        responses: &[FolderResponse],
    ) -> Result<HashMap<FolderId, FolderKey>> {
        let mut rotations: Vec<i64> = responses
            .iter()
            .filter(|folder| folder.parent_folder_id().is_none())
            .map(FolderResponse::key_rotation)
            .collect();
        rotations.sort_unstable();
        rotations.dedup();

        let keys = self.share_keys_for_rotations(share, &rotations).await?;
        decrypt_folder_keys(share, responses, &keys)
    }

    async fn folder_responses(&self, share: &ShareId) -> Result<Vec<FolderResponse>> {
        self.api()
            .get(format!("{PREFIX}/share/{share}/folder"))
            .pages(Paging::since().page_size(PAGE_SIZE))
            .field("Folders")
            .collect(|page: FolderPage| Page::new(page.folders).next(page.last_token))
            .await
            .map_err(Into::into)
    }

    fn decrypt_folder(
        &self,
        share: &ShareId,
        response: FolderResponse,
        key: FolderKey,
    ) -> Result<PassFolder> {
        let folder = crate::crypto::folder::decrypt_content(&key, &response)?;

        Ok(PassFolder::new(share.clone(), response, folder.name, key))
    }
}

fn share_key_by_rotation(
    share: &ShareId,
    keys: &ShareKeys,
    rotation: i64,
) -> Result<EncryptionKey> {
    keys.get(rotation)
        .map(|key| key.key().clone())
        .ok_or_else(|| Error::MissingShareKey {
            share: share.clone(),
            rotation,
        })
}
