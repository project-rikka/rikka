use futures_util::future::{self, BoxFuture};
use futures_util::{FutureExt as _, Stream, StreamExt as _, stream};
use rikka_proton_api::{detached, offload, wire_enum};
use serde::{Deserialize, Serialize};

use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::file::File;
use crate::identifier::{LinkId, ThumbnailId, VolumeId};

pub const THUMBNAIL_MAX_SIDE: u32 = 512;

pub const THUMBNAIL_MAX_SIZE: usize = 60 * 1024;

pub const HD_THUMBNAIL_MAX_SIDE: u32 = 1920;

pub const HD_THUMBNAIL_MAX_SIZE: usize = 1024 * 1024;

const THUMBNAILS_PER_REQUEST: usize = 30;
const CONCURRENT_REQUESTS: usize = 2;
const CONCURRENT_THUMBNAILS: usize = 12;

wire_enum! {
    pub enum ThumbnailType: default Preview {
        Preview = 1,
        HighDefinition = 2,
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThumbnailInfo {
    #[serde(rename = "ThumbnailID", default)]
    id: ThumbnailId,
    #[serde(rename = "Type", default)]
    kind: ThumbnailType,
    #[serde(rename = "Size", default)]
    size: Option<i64>,
    #[serde(rename = "EncryptedSize", default)]
    encrypted_size: Option<i64>,
    #[serde(rename = "Hash", default)]
    hash: Option<String>,
}

impl ThumbnailInfo {
    #[must_use]
    pub fn id(&self) -> &ThumbnailId {
        &self.id
    }

    #[must_use]
    pub fn kind(&self) -> ThumbnailType {
        self.kind
    }

    #[must_use]
    pub fn size(&self) -> i64 {
        self.encrypted_size.or(self.size).unwrap_or_default()
    }

    #[must_use]
    pub fn hash(&self) -> Option<&str> {
        self.hash.as_deref()
    }
}

impl DriveClient {
    pub async fn thumbnail(&self, volume: &VolumeId, file: &File) -> Result<Option<Vec<u8>>> {
        self.thumbnail_with_kind(volume, file, ThumbnailType::Preview)
            .await
    }

    pub async fn thumbnail_with_kind(
        &self,
        volume: &VolumeId,
        file: &File,
        kind: ThumbnailType,
    ) -> Result<Option<Vec<u8>>> {
        let Some(id) = thumbnail_id(file, kind) else {
            return Ok(None);
        };

        let cache_client = (self.clone(), id.clone());
        if let Some(sealed) =
            offload(move || cache_client.0.stored_thumbnail(&cache_client.1)).await
        {
            return self.decrypt_thumbnail(file, sealed).await.map(Some);
        }

        let response = self.thumbnail_addresses(volume, vec![id.clone()]).await?;
        let (url, token) = response.address(&id)?;

        self.fetch_thumbnail(file, &id, &url, &token)
            .await
            .map(Some)
    }

    pub fn thumbnails(
        &self,
        volume: &VolumeId,
        files: Vec<File>,
    ) -> impl Stream<Item = (LinkId, Result<Option<Vec<u8>>>)> + Send + 'static {
        self.thumbnails_with_kind(volume, files, ThumbnailType::Preview)
    }

    pub fn thumbnails_with_kind(
        &self,
        volume: &VolumeId,
        files: Vec<File>,
        kind: ThumbnailType,
    ) -> impl Stream<Item = (LinkId, Result<Option<Vec<u8>>>)> + Send + 'static {
        let mut without_thumbnail = Vec::new();
        let mut with_ids = Vec::new();
        for file in files {
            match thumbnail_id(&file, kind) {
                Some(id) => with_ids.push((file, id)),
                None => without_thumbnail.push(file.id().clone()),
            }
        }

        let batches: Vec<Vec<(File, ThumbnailId)>> = with_ids
            .chunks(THUMBNAILS_PER_REQUEST)
            .map(<[(File, ThumbnailId)]>::to_vec)
            .collect();
        let (client, volume) = (self.clone(), volume.clone());

        stream::iter(
            without_thumbnail
                .into_iter()
                .map(|id| future::ready((id, Ok(None))).boxed()),
        )
        .chain(
            stream::iter(batches)
                .map(move |batch| fetch_batch(client.clone(), volume.clone(), batch))
                .buffered(CONCURRENT_REQUESTS)
                .flat_map(stream::iter),
        )
        .buffer_unordered(CONCURRENT_THUMBNAILS)
    }

    async fn thumbnail_addresses(
        &self,
        volume: &VolumeId,
        ids: Vec<ThumbnailId>,
    ) -> Result<AddressesResponse> {
        Ok(self
            .api()
            .post(format!("drive/volumes/{volume}/thumbnails"))
            .json(&AddressesRequest { ids })
            .send()
            .await?)
    }

    async fn fetch_thumbnail(
        &self,
        file: &File,
        id: &ThumbnailId,
        url: &str,
        token: &str,
    ) -> Result<Vec<u8>> {
        let sealed = self.fetch_stored(url, Some(token)).await?;
        let decrypted = self.decrypt_thumbnail(file, sealed.to_vec()).await?;
        self.cache_thumbnail(id, &sealed);
        Ok(decrypted)
    }

    async fn decrypt_thumbnail(&self, file: &File, sealed: Vec<u8>) -> Result<Vec<u8>> {
        let (pgp, content_key) = (*self.pgp(), file.content_key()?.clone());

        offload(move || crypto::decrypt_block(&pgp, &content_key, &sealed)).await
    }
}

type ThumbnailFetch = BoxFuture<'static, (LinkId, Result<Option<Vec<u8>>>)>;

fn thumbnail_id(file: &File, kind: ThumbnailType) -> Option<ThumbnailId> {
    file.active_revision()
        .and_then(|revision| revision.thumbnail(kind))
        .map(|thumbnail| thumbnail.id().clone())
        .filter(|id| !id.is_empty())
}

async fn fetch_batch(
    client: DriveClient,
    volume: VolumeId,
    batch: Vec<(File, ThumbnailId)>,
) -> Vec<ThumbnailFetch> {
    let cache_client = client.clone();
    let (cached, batch) = offload(move || {
        let mut cached = Vec::new();
        let mut missing = Vec::new();
        for (file, id) in batch {
            match cache_client.stored_thumbnail(&id) {
                Some(sealed) => cached.push((file, sealed)),
                None => missing.push((file, id)),
            }
        }
        (cached, missing)
    })
    .await;
    let mut fetches: Vec<ThumbnailFetch> = cached
        .into_iter()
        .map(|(file, sealed)| decrypt_cached_thumbnail(client.clone(), file, sealed))
        .collect();
    if batch.is_empty() {
        return fetches;
    }

    let ids = batch.iter().map(|(_, id)| id.clone()).collect();
    let response = match client.thumbnail_addresses(&volume, ids).await {
        Ok(response) => response,
        Err(error) => {
            let reason = error.to_string();
            fetches.extend(batch.into_iter().map(|(file, id)| {
                let failed = Error::ThumbnailRefused {
                    id,
                    reason: reason.clone(),
                };
                future::ready((file.id().clone(), Err(failed))).boxed()
            }));
            return fetches;
        }
    };

    fetches.extend(
        batch
            .into_iter()
            .map(|(file, id)| match response.address(&id) {
                Ok((url, token)) => fetch_thumbnail_task(client.clone(), file, id, url, token),
                Err(refused) => future::ready((file.id().clone(), Err(refused))).boxed(),
            }),
    );
    fetches
}

fn decrypt_cached_thumbnail(client: DriveClient, file: File, sealed: Vec<u8>) -> ThumbnailFetch {
    async move {
        let id = file.id().clone();
        (id, client.decrypt_thumbnail(&file, sealed).await.map(Some))
    }
    .boxed()
}

fn fetch_thumbnail_task(
    client: DriveClient,
    file: File,
    thumbnail: ThumbnailId,
    url: String,
    token: String,
) -> ThumbnailFetch {
    async move {
        let id = file.id().clone();
        let read = detached(async move {
            client
                .fetch_thumbnail(&file, &thumbnail, &url, &token)
                .await
                .map(Some)
        })
        .await;
        (id, read)
    }
    .boxed()
}

#[derive(Debug, Serialize)]
struct AddressesRequest {
    #[serde(rename = "ThumbnailIDs")]
    ids: Vec<ThumbnailId>,
}

#[derive(Debug, Deserialize)]
struct AddressesResponse {
    #[serde(rename = "Thumbnails", default)]
    thumbnails: Vec<ThumbnailAddress>,
    #[serde(rename = "Errors", default)]
    errors: Vec<ThumbnailFailure>,
}

impl AddressesResponse {
    fn address(&self, id: &ThumbnailId) -> Result<(String, String)> {
        if let Some(thumbnail) = self.thumbnails.iter().find(|thumbnail| &thumbnail.id == id) {
            return Ok((thumbnail.bare_url.clone(), thumbnail.token.clone()));
        }
        match self.errors.iter().find(|failed| &failed.id == id) {
            Some(failed) => Err(Error::ThumbnailRefused {
                id: id.clone(),
                reason: failed.error.clone(),
            }),
            None => Err(Error::Incomplete("thumbnail address")),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ThumbnailAddress {
    #[serde(rename = "ThumbnailID")]
    id: ThumbnailId,
    #[serde(rename = "BareURL")]
    bare_url: String,
    #[serde(rename = "Token")]
    token: String,
}

#[derive(Debug, Deserialize)]
struct ThumbnailFailure {
    #[serde(rename = "ThumbnailID")]
    id: ThumbnailId,
    #[serde(rename = "Error", default)]
    error: String,
}
