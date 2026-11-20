mod block_stream;
mod draft;
mod photo;
mod resume;
mod source;
mod verifier;
mod wire;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bytes::Bytes;
use futures_util::future;
use jiff::Timestamp;

use crate::attributes::ExtendedAttributes;
use crate::client::DriveClient;
use crate::error::{Error, Result};
use crate::file::File;
use crate::folder::Folder;
use crate::share_keys::ShareKeys;
use crate::thumbnail::ThumbnailType;
use crate::upload::block_stream::{Batch, BlockStream};
use crate::upload::draft::{Commit, Draft, DraftContext, Target};
use crate::upload::resume::Reporter;
use crate::upload::source::Source;

pub use photo::PhotoDetails;
pub use resume::{ResumePoint, UploadProgress, UploadedBlock};

pub const BLOCK_SIZE: usize = 4 * 1024 * 1024;

impl DriveClient {
    pub fn upload<'a>(
        &'a self,
        share: &'a ShareKeys,
        parent: &'a Folder,
        name: impl Into<String>,
    ) -> Upload<'a> {
        Upload::new(self, share, parent, Destination::File(name.into()))
    }

    pub fn upload_revision<'a>(
        &'a self,
        share: &'a ShareKeys,
        parent: &'a Folder,
        file: &'a File,
    ) -> Upload<'a> {
        Upload::new(self, share, parent, Destination::Revision(file))
    }
}

#[derive(Debug)]
enum Destination<'a> {
    File(String),
    Revision(&'a File),
}

#[must_use]
#[derive(Debug)]
pub struct Upload<'a> {
    client: &'a DriveClient,
    share: &'a ShareKeys,
    parent: &'a Folder,
    destination: Destination<'a>,
    media_type: Option<String>,
    modified_at: Option<Timestamp>,
    thumbnails: Vec<(ThumbnailType, Vec<u8>)>,
    attributes: ExtendedAttributes,
    expected_sha1: Option<String>,
    photo: Option<PhotoDetails>,
    reporter: Option<Reporter<'a>>,
    resume: Option<ResumePoint>,
}

#[derive(Debug)]
enum Origin {
    Bytes(Bytes),
    Path(PathBuf),
}

impl Origin {
    async fn open(&self) -> Result<Source> {
        Ok(match self {
            Origin::Bytes(bytes) => Source::Bytes(bytes.clone()),
            Origin::Path(path) => Source::File(tokio::fs::File::open(path).await?),
        })
    }
}

impl<'a> Upload<'a> {
    fn new(
        client: &'a DriveClient,
        share: &'a ShareKeys,
        parent: &'a Folder,
        destination: Destination<'a>,
    ) -> Upload<'a> {
        Upload {
            client,
            share,
            parent,
            destination,
            media_type: None,
            modified_at: None,
            thumbnails: Vec::new(),
            attributes: ExtendedAttributes::new(),
            expected_sha1: None,
            photo: None,
            reporter: None,
            resume: None,
        }
    }

    pub fn media_type(mut self, media_type: impl Into<String>) -> Self {
        self.media_type = Some(media_type.into());
        self
    }

    pub fn modified_at(mut self, at: Timestamp) -> Self {
        self.modified_at = Some(at);
        self
    }

    pub fn thumbnail(self, picture: impl Into<Vec<u8>>) -> Self {
        self.thumbnail_with_kind(ThumbnailType::Preview, picture)
    }

    pub fn hd_thumbnail(self, picture: impl Into<Vec<u8>>) -> Self {
        self.thumbnail_with_kind(ThumbnailType::HighDefinition, picture)
    }

    pub fn thumbnail_with_kind(mut self, kind: ThumbnailType, picture: impl Into<Vec<u8>>) -> Self {
        self.thumbnails
            .retain(|(thumbnail_kind, _)| *thumbnail_kind != kind);
        self.thumbnails.push((kind, picture.into()));
        self.thumbnails.sort_by_key(|(kind, _)| i32::from(*kind));
        self
    }

    pub fn attributes(mut self, attributes: ExtendedAttributes) -> Self {
        self.attributes = attributes;
        self
    }

    pub fn expected_sha1(mut self, digest: impl Into<String>) -> Self {
        self.expected_sha1 = Some(digest.into().to_ascii_lowercase());
        self
    }

    pub fn photo(mut self, details: PhotoDetails) -> Self {
        self.photo = Some(details);
        self
    }

    pub fn on_progress(mut self, report: impl FnMut(UploadProgress) + Send + 'a) -> Self {
        self.reporter = Some(Reporter::new(report));
        self
    }

    pub fn resume_from(mut self, point: ResumePoint) -> Self {
        self.resume = Some(point);
        self
    }

    pub async fn upload_bytes(self, data: impl Into<Bytes>) -> Result<File> {
        self.run(Origin::Bytes(data.into())).await
    }

    pub async fn upload_file(self, path: impl AsRef<Path>) -> Result<File> {
        let origin = Origin::Path(path.as_ref().to_owned());
        origin.open().await?;

        self.run(origin).await
    }

    async fn run(mut self, origin: Origin) -> Result<File> {
        let media_type = self.media_type_or_default();
        let mut reporter = self.reporter.take();
        let context = DraftContext {
            client: self.client,
            share: self.share,
            parent: self.parent,
        };
        let resumed = match self.resume.take() {
            Some(point) => Draft::resume(context, self.target(&media_type), &point).await?,
            None => None,
        };
        let (draft, uploaded_blocks) = match resumed {
            Some(resumed) => resumed,
            None => {
                let draft = Draft::create(context, self.target(&media_type)).await?;
                (draft, HashMap::new())
            }
        };
        if let Some(reporter) = &mut reporter {
            reporter.report(UploadProgress::DraftCreated {
                link: draft.link().clone(),
                revision: draft.revision().clone(),
            });
        }

        let resumable = reporter.is_some();
        match self
            .transfer(&draft, origin.open().await?, uploaded_blocks, &mut reporter)
            .await
        {
            Ok(()) => draft.fetch_file().await,
            Err(error) => {
                if !resumable || matches!(error, Error::ChecksumMismatch { .. }) {
                    draft.discard().await;
                }
                Err(error)
            }
        }
    }

    fn target<'t>(&'t self, media_type: &'t str) -> Target<'t> {
        match &self.destination {
            Destination::File(name) => Target::File { name, media_type },
            Destination::Revision(file) => Target::Revision(file),
        }
    }

    async fn transfer(
        &self,
        draft: &Draft<'_>,
        source: Source,
        uploaded_blocks: HashMap<u32, UploadedBlock>,
        reporter: &mut Option<Reporter<'_>>,
    ) -> Result<()> {
        let mut thumbnails = self
            .thumbnails
            .iter()
            .map(|(kind, picture)| draft.seal_thumbnail(*kind, picture))
            .collect::<Result<Vec<_>>>()?;
        let mut blocks = BlockStream::new(source, draft.sealer(), uploaded_blocks);
        let mut thumbnail_hashes = Vec::new();
        let Batch(mut batch) = blocks.next_batch().await?;

        loop {
            let pending_thumbnails = std::mem::take(&mut thumbnails);
            if batch.is_empty() && pending_thumbnails.is_empty() {
                break;
            }
            let uploaded: Vec<UploadedBlock> = batch.iter().map(|chunk| chunk.uploaded()).collect();
            let (sent_thumbnails, Batch(next)) =
                future::try_join(draft.send(batch, pending_thumbnails), blocks.next_batch())
                    .await?;
            if let Some(reporter) = reporter {
                for block in uploaded {
                    reporter.report(UploadProgress::BlockUploaded(block));
                }
            }
            thumbnail_hashes.extend(sent_thumbnails);
            batch = next;
        }

        thumbnail_hashes.sort_by_key(|(kind, _)| i32::from(*kind));
        let hashes: Vec<[u8; 32]> = thumbnail_hashes.into_iter().map(|(_, hash)| hash).collect();
        let sealed = blocks.finish(&hashes);
        let checksum_verified = match &self.expected_sha1 {
            Some(expected) if *expected != sealed.sha1 => {
                return Err(Error::ChecksumMismatch {
                    expected: expected.clone(),
                    computed: sealed.sha1,
                });
            }
            Some(_) => true,
            None => false,
        };

        draft
            .commit(Commit {
                sealed,
                attributes: self.attributes.clone(),
                modified_at: self.modified_at,
                checksum_verified,
                photo: self.photo.as_ref(),
            })
            .await
    }
    fn media_type_or_default(&self) -> String {
        if let Some(selected) = &self.media_type {
            return selected.clone();
        }
        match &self.destination {
            Destination::File(name) => mime_guess::from_path(name)
                .first_or_octet_stream()
                .to_string(),
            Destination::Revision(file) => file.media_type().to_owned(),
        }
    }
}
