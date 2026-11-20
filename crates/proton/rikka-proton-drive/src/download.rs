use std::path::{Path, PathBuf};
use std::pin::pin;

use futures_util::stream::{self, BoxStream};
use futures_util::{Stream, StreamExt as _, TryStreamExt as _, future};
use rikka_proton_api::{detached, offload};
use rikka_proton_pgp::SessionKey;

use crate::blocks::DecryptedBlock;
use crate::client::DriveClient;
use crate::crypto;
use crate::error::{Error, Result};
use crate::file::File;
use crate::identifier::{LinkId, RevisionId, VolumeId};
use crate::revision::{Block, Revision, RevisionInfo};
use crate::saving::PartialFile;

const BLOCKS_PER_PAGE: u32 = 50;
const FROM_BLOCK_INDEX: &str = "FromBlockIndex";
const PAGE_SIZE: &str = "PageSize";
const CONCURRENT_BLOCKS: usize = 6;

impl DriveClient {
    pub async fn revision_info(
        &self,
        volume: &VolumeId,
        link: &LinkId,
        revision: &RevisionId,
    ) -> Result<RevisionInfo> {
        let mut info = self.revision_page(volume, link, revision, 1).await?;
        let first = info.drain_blocks();
        let blocks = listed_blocks(
            self.clone(),
            volume.clone(),
            link.clone(),
            revision.clone(),
            first,
        )
        .try_collect()
        .await?;

        Ok(info.with_blocks(blocks))
    }

    pub async fn revision(
        &self,
        volume: &VolumeId,
        file: &File,
        revision: &RevisionId,
    ) -> Result<Revision> {
        let info = self.revision_info(volume, file.id(), revision).await?;
        let node_key = file.keys().key();

        let Some(armored) = info.extended_attributes() else {
            return Ok(Revision::new(
                info,
                None,
                rikka_proton_pgp::SignatureStatus::Missing,
            ));
        };

        let verification = self.verification_keys_or(info.signature_email(), node_key.public_key());
        let (attributes, signature) =
            crypto::decrypt_attributes(self.pgp(), node_key, &verification, armored)?;
        let signature = Error::check_signature("extended attributes", signature)?;

        Ok(Revision::new(info, Some(attributes), signature))
    }

    pub fn download<'a>(&'a self, volume: &'a VolumeId, file: &'a File) -> Download<'a> {
        Download {
            client: self,
            volume,
            file,
            revision: None,
        }
    }

    async fn revision_page(
        &self,
        volume: &VolumeId,
        link: &LinkId,
        revision: &RevisionId,
        from: u32,
    ) -> Result<RevisionInfo> {
        Ok(self
            .api()
            .get(revision_path(volume, link, revision))
            .query(&[
                (FROM_BLOCK_INDEX, from.to_string()),
                (PAGE_SIZE, BLOCKS_PER_PAGE.to_string()),
            ])
            .send_field("Revision")
            .await?)
    }
}

#[must_use]
#[derive(Debug)]
pub struct Download<'a> {
    client: &'a DriveClient,
    volume: &'a VolumeId,
    file: &'a File,
    revision: Option<RevisionId>,
}

impl<'a> Download<'a> {
    pub fn revision(mut self, revision: RevisionId) -> Self {
        self.revision = Some(revision);
        self
    }

    pub fn into_stream(self) -> impl Stream<Item = Result<Vec<u8>>> + Send + 'a {
        stream::unfold(DownloadState::Start(self), DownloadState::step)
    }

    pub async fn into_vec(self) -> Result<Vec<u8>> {
        self.into_stream().try_concat().await
    }

    pub async fn save_into(
        self,
        folder: impl AsRef<Path>,
        mut progress: impl FnMut(u64) + Send,
    ) -> Result<PathBuf> {
        let name = self.file.name().to_owned();
        let mut partial = PartialFile::create(folder.as_ref()).await?;
        let mut blocks = pin!(self.into_stream());
        let mut bytes_written = 0u64;

        while let Some(block) = blocks.try_next().await? {
            partial.write(&block).await?;
            bytes_written += block.len() as u64;
            progress(bytes_written);
        }

        Ok(partial.place(&name).await?)
    }

    pub async fn for_each_block(
        self,
        mut sink: impl FnMut(Vec<u8>) -> Result<()> + Send,
    ) -> Result<u64> {
        let mut blocks = pin!(self.into_stream());
        let mut bytes_written = 0u64;

        while let Some(block) = blocks.try_next().await? {
            bytes_written += block.len() as u64;
            sink(block)?;
        }

        Ok(bytes_written)
    }

    async fn decrypted_blocks(
        &self,
    ) -> Result<(RevisionInfo, BoxStream<'static, Result<DecryptedBlock>>)> {
        let revision = match &self.revision {
            Some(revision) => revision.clone(),
            None => self.file.revision_id()?.clone(),
        };
        let content_key = self.file.content_key()?.clone();

        let mut info = self
            .client
            .revision_page(self.volume, self.file.id(), &revision, 1)
            .await?;
        let first = info.drain_blocks();
        let listed = listed_blocks(
            self.client.clone(),
            self.volume.clone(),
            self.file.id().clone(),
            revision,
            first,
        );

        Ok((
            info,
            decrypt_blocks(self.client.clone(), listed, content_key),
        ))
    }

    fn verify_manifest(&self, info: &RevisionInfo, manifest: &[u8]) -> Result<()> {
        let Some(armored) = info.manifest_signature() else {
            return Ok(());
        };
        let verification = self
            .client
            .verification_keys_or(info.signature_email(), self.file.keys().key().public_key());
        let status = crypto::verify_manifest(self.client.pgp(), &verification, manifest, armored);
        Error::check_signature("manifest", status).map(drop)
    }
}

enum DownloadState<'a> {
    Start(Download<'a>),
    Blocks(Box<Blocks<'a>>),
    Done,
}

struct Blocks<'a> {
    download: Download<'a>,
    info: RevisionInfo,
    manifest: Vec<u8>,
    decrypted_blocks: BoxStream<'static, Result<DecryptedBlock>>,
}

impl<'a> DownloadState<'a> {
    async fn step(self) -> Option<(Result<Vec<u8>>, DownloadState<'a>)> {
        let mut blocks = match self {
            DownloadState::Done => return None,
            DownloadState::Blocks(blocks) => blocks,
            DownloadState::Start(download) => match start_blocks(download).await {
                Ok(blocks) => blocks,
                Err(error) => return Some((Err(error), DownloadState::Done)),
            },
        };

        match blocks.decrypted_blocks.next().await {
            Some(Ok(block)) => {
                blocks.manifest.extend_from_slice(&block.digest);
                Some((Ok(block.plaintext), DownloadState::Blocks(blocks)))
            }
            Some(Err(error)) => Some((Err(error), DownloadState::Done)),
            None => match blocks
                .download
                .verify_manifest(&blocks.info, &blocks.manifest)
            {
                Ok(()) => None,
                Err(error) => Some((Err(error), DownloadState::Done)),
            },
        }
    }
}

async fn start_blocks(download: Download<'_>) -> Result<Box<Blocks<'_>>> {
    let (info, decrypted_blocks) = download.decrypted_blocks().await?;
    let manifest = thumbnail_manifest(&info)?;

    Ok(Box::new(Blocks {
        download,
        info,
        manifest,
        decrypted_blocks,
    }))
}

fn listed_blocks(
    client: DriveClient,
    volume: VolumeId,
    link: LinkId,
    revision: RevisionId,
    first: Vec<Block>,
) -> impl Stream<Item = Result<Block>> + Send + 'static {
    let remaining = stream::unfold(next_block_index(&first), move |from| {
        let (client, volume, link, revision) = (
            client.clone(),
            volume.clone(),
            link.clone(),
            revision.clone(),
        );
        async move {
            let from = from?;
            match client.revision_page(&volume, &link, &revision, from).await {
                Ok(mut page) => {
                    let blocks = page.drain_blocks();
                    let next = next_block_index(&blocks);
                    (!blocks.is_empty()).then_some((Ok(blocks), next))
                }
                Err(error) => Some((Err(error), None)),
            }
        }
    });

    stream::once(future::ok(first))
        .chain(remaining)
        .map_ok(|blocks| stream::iter(blocks.into_iter().map(Ok)))
        .try_flatten()
}

fn decrypt_blocks(
    client: DriveClient,
    listed: impl Stream<Item = Result<Block>> + Send + 'static,
    content_key: SessionKey,
) -> BoxStream<'static, Result<DecryptedBlock>> {
    listed
        .map(move |block| {
            let (client, content_key) = (client.clone(), content_key.clone());
            detached(async move {
                let block = block?;
                let sealed = client.fetch_block(&block).await?;
                let pgp = *client.pgp();
                offload(move || DecryptedBlock::decrypt(&pgp, &content_key, &block, &sealed)).await
            })
        })
        .buffered(CONCURRENT_BLOCKS)
        .boxed()
}

fn next_block_index(blocks: &[Block]) -> Option<u32> {
    match blocks.len() >= BLOCKS_PER_PAGE as usize {
        true => blocks.last().map(|block| block.index() + 1),
        false => None,
    }
}

fn thumbnail_manifest(info: &RevisionInfo) -> Result<Vec<u8>> {
    let mut manifest = Vec::new();
    for thumbnail in info.thumbnails() {
        let Some(hash) = thumbnail.hash() else {
            continue;
        };
        let raw = rikka_proton_api::base64::decode(hash)
            .map_err(|_| Error::NotBase64("thumbnail hash"))?;
        manifest.extend_from_slice(&raw);
    }
    Ok(manifest)
}

fn revision_path(volume: &VolumeId, link: &LinkId, revision: &RevisionId) -> String {
    format!("drive/v2/volumes/{volume}/files/{link}/revisions/{revision}")
}
