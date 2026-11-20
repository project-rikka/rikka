use std::fmt;
use std::path::Path;
use std::pin::pin;
use std::time::Duration;

use futures_util::{Stream, StreamExt as _, TryStreamExt as _, stream};
use rikka_proton_api::Api;
use tokio::io::AsyncWriteExt as _;

use crate::api::FileChunk;
use crate::client::PREFIX;
use crate::crypto::EncryptionKey;
use crate::error::Result;
use crate::identifier::{ChunkId, FileId, ItemId, ShareId};

pub(crate) const TRANSFER_TIMEOUT: Duration = Duration::from_secs(90);
const PREFETCH: usize = 2;

#[derive(Debug, Clone)]
pub(crate) enum ChunkRoute {
    Item {
        share: ShareId,
        item: ItemId,
        file: FileId,
    },
    Public {
        token: String,
        file: FileId,
    },
}

impl ChunkRoute {
    fn path(&self, chunk: &ChunkId) -> String {
        match self {
            ChunkRoute::Item { share, item, file } => {
                format!("{PREFIX}/share/{share}/item/{item}/file/{file}/chunk/{chunk}")
            }
            ChunkRoute::Public { token, file } => {
                format!("{PREFIX}/public_link/files/{token}/{file}/{chunk}")
            }
        }
    }
}

#[must_use]
pub struct Download {
    api: Api,
    route: ChunkRoute,
    key: EncryptionKey,
    version: i32,
    chunks: Vec<FileChunk>,
}

impl Download {
    pub(crate) fn new(
        api: Api,
        route: ChunkRoute,
        key: EncryptionKey,
        version: i32,
        chunks: &[FileChunk],
    ) -> Download {
        let mut chunks = chunks.to_vec();
        chunks.sort_by_key(FileChunk::index);

        Download {
            api,
            route,
            key,
            version,
            chunks,
        }
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<Vec<u8>>> + Send + 'static {
        let Download {
            api,
            route,
            key,
            version,
            chunks,
        } = self;
        let total = chunks.len();

        stream::iter(chunks)
            .map(move |chunk| {
                let (api, path, key) = (api.clone(), route.path(chunk.id()), key.clone());
                async move {
                    let sealed = api
                        .get(path)
                        .header("Accept", "application/octet-stream")
                        .timeout(TRANSFER_TIMEOUT)
                        .send_bytes()
                        .await?;

                    rikka_proton_api::offload(move || {
                        crate::crypto::file::decrypt_chunk(
                            &key,
                            &sealed,
                            chunk.index(),
                            total,
                            version,
                        )
                    })
                    .await
                }
            })
            .buffered(PREFETCH)
    }

    pub async fn into_vec(self) -> Result<Vec<u8>> {
        self.into_stream().try_concat().await
    }

    pub async fn write_to_file(self, path: impl AsRef<Path>) -> Result<u64> {
        let mut file = tokio::fs::File::create(path.as_ref()).await?;
        let mut chunks = pin!(self.into_stream());
        let mut bytes_written = 0u64;

        while let Some(chunk) = chunks.try_next().await? {
            file.write_all(&chunk).await?;
            bytes_written += chunk.len() as u64;
        }
        file.flush().await?;

        Ok(bytes_written)
    }
}

impl fmt::Debug for Download {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Download")
            .field("chunks", &self.chunks.len())
            .finish_non_exhaustive()
    }
}
