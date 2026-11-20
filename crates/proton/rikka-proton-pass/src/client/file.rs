use std::fmt;
use std::path::Path;

use bytes::Bytes;
use futures_util::future;
use rikka_proton_api::{Multipart, Page, Paging};
use tokio::io::AsyncReadExt as _;

use crate::ItemRevision;
use crate::api::RestoredFiles;
use crate::api::{
    CreatePendingFileRequest, EncryptedFileKey, FileMetadataRequest, FilePage, FileResponse,
    LinkFilesRequest, RestoreFilesRequest,
};
use crate::client::{LARGE_BATCH, PREFIX, PassClient, SMALL_BATCH};
use crate::content::FileMetadata;
use crate::crypto::file::ENCRYPTION_VERSION;
use crate::crypto::{EncryptionKey, ItemKey};
use crate::download::{ChunkRoute, Download, TRANSFER_TIMEOUT};
use crate::error::{Error, Result};
use crate::identifier::{FileId, ItemId, ShareId};
use crate::pass_file::PassFile;
use crate::pass_item::PassItem;
use crate::pending_file::PendingFile;

const CHUNK_SIZE: usize = 4 * 1024 * 1024;
const RESTORE_BATCH: usize = 50;

impl PassClient {
    pub fn attach(&self, name: impl Into<String>) -> Attachment<'_> {
        Attachment {
            client: self,
            name: name.into(),
            media_type: None,
        }
    }

    pub async fn rename_pending_file(
        &self,
        file: &PendingFile,
        name: impl Into<String>,
    ) -> Result<PendingFile> {
        let metadata = FileMetadata {
            name: name.into(),
            mime_type: file.mime_type().unwrap_or_default().to_owned(),
        };
        let request = FileMetadataRequest {
            metadata: crate::crypto::file::encrypt_metadata(
                file.key(),
                &metadata,
                ENCRYPTION_VERSION,
            )?,
        };

        let id = file.id();
        let _: FileId = self
            .api()
            .put(format!("{PREFIX}/file/{id}/metadata"))
            .json(&request)
            .send_field_at(&["File", "FileID"])
            .await?;

        Ok(file.with_metadata(metadata))
    }

    pub async fn link_files(
        &self,
        item: &PassItem,
        add: &[PendingFile],
        remove: &[FileId],
    ) -> Result<PassItem> {
        let sealed = add
            .iter()
            .map(|file| {
                Ok(EncryptedFileKey {
                    id: file.id().clone(),
                    key: crate::crypto::file::encrypt_file_key(item.key().key(), file.key())?,
                })
            })
            .collect::<Result<Vec<EncryptedFileKey>>>()?;

        let mut adding = sealed.chunks(SMALL_BATCH);
        let mut removing = remove.chunks(LARGE_BATCH);
        let (share, id) = (item.share_id(), item.id());
        let mut current = item.clone();

        loop {
            let (added, removed) = (adding.next(), removing.next());
            if added.is_none() && removed.is_none() {
                return Ok(current);
            }

            let request = LinkFilesRequest {
                revision: current.revision(),
                add: added.unwrap_or_default().to_vec(),
                remove: removed.unwrap_or_default().to_vec(),
            };
            let envelope: ItemRevision = self
                .api()
                .post(format!("{PREFIX}/share/{share}/item/{id}/link_files"))
                .json(&request)
                .send_field("Item")
                .await?;

            current = self.decrypt_item(share, envelope).await?;
        }
    }

    pub async fn files(&self, item: &PassItem) -> Result<Vec<PassFile>> {
        let (share, id) = (item.share_id(), item.id());
        let responses = self
            .paged_files(format!("{PREFIX}/share/{share}/item/{id}/files"))
            .await?;

        self.decrypt_files(item, responses).await
    }

    pub async fn file_history(&self, item: &PassItem) -> Result<Vec<PassFile>> {
        let (share, id) = (item.share_id(), item.id());
        let responses = self
            .paged_files(format!("{PREFIX}/share/{share}/item/{id}/revisions/files"))
            .await?;

        self.decrypt_files(item, responses).await
    }

    pub async fn rename_file(&self, file: &PassFile, name: impl Into<String>) -> Result<PassFile> {
        let metadata = FileMetadata {
            name: name.into(),
            mime_type: file.metadata().mime_type.clone(),
        };
        let request = FileMetadataRequest {
            metadata: crate::crypto::file::encrypt_metadata(
                file.key(),
                &metadata,
                file.response().encryption_version(),
            )?,
        };

        let (share, item, id) = (file.share_id(), file.item_id(), file.id());
        let envelope: FileResponse = self
            .api()
            .put(format!(
                "{PREFIX}/share/{share}/item/{item}/file/{id}/metadata"
            ))
            .json(&request)
            .send_field("File")
            .await?;

        Ok(PassFile::new(
            share.clone(),
            item.clone(),
            envelope,
            metadata,
            file.key().clone(),
        ))
    }

    pub async fn restore_files(
        &self,
        item: &PassItem,
        files: &[PassFile],
    ) -> Result<(PassItem, Vec<PassFile>)> {
        let item_key = item.key();
        let sealed = files
            .iter()
            .map(|file| {
                Ok(EncryptedFileKey {
                    id: file.id().clone(),
                    key: crate::crypto::file::encrypt_file_key(item_key.key(), file.key())?,
                })
            })
            .collect::<Result<Vec<EncryptedFileKey>>>()?;

        let (share, id) = (item.share_id(), item.id());
        let mut current = item.clone();
        let mut restored = Vec::with_capacity(files.len());

        for batch in sealed.chunks(RESTORE_BATCH) {
            let request = RestoreFilesRequest {
                files: batch.to_vec(),
                item_key_rotation: item_key.rotation(),
            };
            let envelope: RestoredFiles = self
                .api()
                .post(format!("{PREFIX}/share/{share}/item/{id}/file/restore"))
                .json(&request)
                .send_field("Result")
                .await?;

            current = self.decrypt_item(share, envelope.item).await?;
            restored.extend(envelope.files);
        }

        let decrypted = self.decrypt_files(&current, restored).await?;
        Ok((current, decrypted))
    }

    pub fn download_file(&self, file: &PassFile) -> Download {
        Download::new(
            self.api().clone(),
            ChunkRoute::Item {
                share: file.share_id().clone(),
                item: file.item_id().clone(),
                file: file.id().clone(),
            },
            file.key().clone(),
            file.response().encryption_version(),
            file.chunks(),
        )
    }

    async fn paged_files(&self, path: String) -> Result<Vec<FileResponse>> {
        Ok(self
            .api()
            .get(path)
            .pages(Paging::since())
            .field("Files")
            .collect(|page: FilePage| Page::new(page.files).next(page.last_id))
            .await?)
    }

    async fn decrypt_files(
        &self,
        item: &PassItem,
        responses: Vec<FileResponse>,
    ) -> Result<Vec<PassFile>> {
        if responses.is_empty() {
            return Ok(Vec::new());
        }

        let current_rotation = item.key().rotation();
        let keys = if responses
            .iter()
            .all(|response| response.item_key_rotation() == current_rotation)
        {
            vec![item.key().clone()]
        } else {
            self.item_keys(item.share_id(), item.id()).await?
        };

        let (share, id) = (item.share_id().clone(), item.id().clone());
        rikka_proton_api::parallel_map(responses, move |response| {
            decrypt_file(&share, &id, &keys, response)
        })
        .await
        .into_iter()
        .collect()
    }
}

fn decrypt_file(
    share: &ShareId,
    item: &ItemId,
    keys: &[ItemKey],
    response: FileResponse,
) -> Result<PassFile> {
    let rotation = response.item_key_rotation();
    let item_key = keys
        .iter()
        .find(|key| key.rotation() == rotation)
        .ok_or_else(|| Error::MissingItemKey {
            item: item.clone(),
            rotation,
        })?;

    let key = crate::crypto::file::decrypt_file_key(item_key.key(), response.file_key())?;
    let metadata = crate::crypto::file::decrypt_metadata(
        &key,
        response.metadata(),
        response.encryption_version(),
    )?;

    Ok(PassFile::new(
        share.clone(),
        item.clone(),
        response,
        metadata,
        key,
    ))
}

#[must_use]
pub struct Attachment<'a> {
    client: &'a PassClient,
    name: String,
    media_type: Option<String>,
}

impl Attachment<'_> {
    pub fn media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    pub async fn upload_bytes(self, data: impl Into<Bytes>) -> Result<PendingFile> {
        let data = data.into();
        let size = data.len() as u64;

        self.upload(UploadSource::Bytes(data), size).await
    }

    pub async fn upload_file(self, path: impl AsRef<Path>) -> Result<PendingFile> {
        let file = tokio::fs::File::open(path.as_ref()).await?;
        let size = file.metadata().await?.len();

        self.upload(UploadSource::File(file), size).await
    }

    async fn upload(self, mut source: UploadSource, size: u64) -> Result<PendingFile> {
        if size == 0 {
            return Err(Error::EmptyFile);
        }

        let chunks = usize::try_from(size.div_ceil(CHUNK_SIZE as u64)).map_err(|_| {
            Error::FileSizeMismatch {
                expected: size,
                read: 0,
            }
        })?;
        let metadata = FileMetadata {
            mime_type: self.resolved_media_type(),
            name: self.name,
        };
        let key = EncryptionKey::generate();
        let client = self.client;

        let request = CreatePendingFileRequest {
            metadata: crate::crypto::file::encrypt_metadata(&key, &metadata, ENCRYPTION_VERSION)?,
            chunk_count: chunks,
            encryption_version: ENCRYPTION_VERSION,
        };
        let id: FileId = client
            .api()
            .post(format!("{PREFIX}/file"))
            .json(&request)
            .send_field_at(&["File", "FileID"])
            .await?;

        let mut read = 0u64;
        let mut next = read_encrypted_chunk(&mut source, &key, 0, chunks).await?;

        for index in 0..chunks {
            let (current, length) = next.take().ok_or(Error::FileSizeMismatch {
                expected: size,
                read,
            })?;
            read += length;

            let upload = client.upload_chunk(&id, index, current);
            if index + 1 < chunks {
                let reading = read_encrypted_chunk(&mut source, &key, index + 1, chunks);
                let ((), following) = future::try_join(upload, reading).await?;
                next = following;
            } else {
                upload.await?;
            }
        }

        if read != size {
            return Err(Error::FileSizeMismatch {
                expected: size,
                read,
            });
        }

        Ok(PendingFile::new(id, metadata, size, chunks, key))
    }

    fn resolved_media_type(&self) -> String {
        self.media_type.clone().unwrap_or_else(|| {
            mime_guess::from_path(&self.name)
                .first_or_octet_stream()
                .essence_str()
                .to_owned()
        })
    }
}

impl fmt::Debug for Attachment<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Attachment")
            .field("media_type", &self.media_type)
            .finish_non_exhaustive()
    }
}

impl PassClient {
    async fn upload_chunk(&self, file: &FileId, index: usize, sealed: Vec<u8>) -> Result<()> {
        let form = Multipart::new().text("ChunkIndex", index.to_string()).file(
            "ChunkData",
            "blob",
            sealed,
        );

        self.api()
            .post(format!("{PREFIX}/file/{file}/chunk"))
            .timeout(TRANSFER_TIMEOUT)
            .multipart(form)
            .send_unit()
            .await?;

        Ok(())
    }
}

enum UploadSource {
    Bytes(Bytes),
    File(tokio::fs::File),
}

impl UploadSource {
    async fn read(&mut self, size: usize) -> Result<Vec<u8>> {
        match self {
            UploadSource::Bytes(bytes) => {
                let taken = size.min(bytes.len());
                Ok(bytes.split_to(taken).to_vec())
            }
            UploadSource::File(file) => {
                let mut chunk = Vec::with_capacity(size);
                (&mut *file)
                    .take(size as u64)
                    .read_to_end(&mut chunk)
                    .await?;
                Ok(chunk)
            }
        }
    }
}

async fn read_encrypted_chunk(
    source: &mut UploadSource,
    key: &EncryptionKey,
    index: usize,
    chunks: usize,
) -> Result<Option<(Vec<u8>, u64)>> {
    let plaintext = source.read(CHUNK_SIZE).await?;
    if plaintext.is_empty() {
        return Ok(None);
    }
    let length = plaintext.len() as u64;

    let key = key.clone();
    let encrypted = rikka_proton_api::offload(move || {
        crate::crypto::file::encrypt_chunk(&key, &plaintext, index, chunks, ENCRYPTION_VERSION)
    })
    .await?;

    Ok(Some((encrypted, length)))
}
