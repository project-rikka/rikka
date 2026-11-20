use rikka_sync::Mutex;
use std::fmt;

use crate::identifier::{LinkId, RevisionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UploadedBlock {
    index: u32,
    hash: [u8; 32],
    size: u64,
    digest: [u8; 32],
}

impl UploadedBlock {
    #[must_use]
    pub fn new(index: u32, hash: [u8; 32], size: u64, digest: [u8; 32]) -> UploadedBlock {
        UploadedBlock {
            index,
            hash,
            size,
            digest,
        }
    }

    #[must_use]
    pub fn index(&self) -> u32 {
        self.index
    }

    #[must_use]
    pub fn hash(&self) -> [u8; 32] {
        self.hash
    }

    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    #[must_use]
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadProgress {
    DraftCreated { link: LinkId, revision: RevisionId },
    BlockUploaded(UploadedBlock),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumePoint {
    link: LinkId,
    revision: RevisionId,
    blocks: Vec<UploadedBlock>,
}

impl ResumePoint {
    #[must_use]
    pub fn new(link: LinkId, revision: RevisionId, blocks: Vec<UploadedBlock>) -> ResumePoint {
        ResumePoint {
            link,
            revision,
            blocks,
        }
    }

    #[must_use]
    pub fn link(&self) -> &LinkId {
        &self.link
    }

    #[must_use]
    pub fn revision(&self) -> &RevisionId {
        &self.revision
    }

    #[must_use]
    pub fn blocks(&self) -> &[UploadedBlock] {
        &self.blocks
    }
}

type Report<'a> = Box<dyn FnMut(UploadProgress) + Send + 'a>;

pub(super) struct Reporter<'a>(Mutex<Report<'a>>);

impl<'a> Reporter<'a> {
    pub(super) fn new(report: impl FnMut(UploadProgress) + Send + 'a) -> Reporter<'a> {
        Reporter(Mutex::new(Box::new(report)))
    }

    pub(super) fn report(&mut self, progress: UploadProgress) {
        let report = self.0.get_mut();
        report(progress);
    }
}

impl fmt::Debug for Reporter<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Reporter")
    }
}
