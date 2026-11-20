use std::path::PathBuf;

use rikka_proton_drive::{BLOCK_SIZE, ResumePoint, UploadProgress, UploadedBlock};
use rikka_proton_drive_sync_state::{DraftBlock, DraftProgress, EntryId};
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

use crate::persist::Persist;
use crate::status::{Direction, ItemState, ItemUpdate};

const MAX_BATCH: usize = 256;

pub(super) struct DraftRecorder {
    sender: mpsc::Sender<DraftProgress>,
    writer: JoinHandle<()>,
}

pub(super) struct UploadedBytes {
    items: broadcast::Sender<ItemUpdate>,
    path: PathBuf,
    total: u64,
}

impl UploadedBytes {
    pub(super) fn new(
        items: broadcast::Sender<ItemUpdate>,
        path: PathBuf,
        resume: Option<&ResumePoint>,
    ) -> UploadedBytes {
        let total = resume.map_or(0, |point| {
            point.blocks().iter().map(UploadedBlock::size).sum()
        });
        UploadedBytes { items, path, total }
    }

    fn add(&mut self, progress: &[DraftProgress]) {
        let added: u64 = progress
            .iter()
            .filter_map(|entry| match entry {
                DraftProgress::BlockUploaded(block) => Some(block.size),
                DraftProgress::Created { .. } => None,
            })
            .sum();
        if added == 0 {
            return;
        }
        self.total += added;
        let state = ItemState::Transferred {
            direction: Direction::Up,
            bytes: self.total,
        };
        self.items
            .send(ItemUpdate::new(self.path.clone(), state))
            .ok();
    }
}

impl DraftRecorder {
    pub(super) fn start(
        persist: Persist,
        entry: EntryId,
        file_size: u64,
        uploaded: UploadedBytes,
    ) -> DraftRecorder {
        let (sender, receiver) = mpsc::channel(progress_capacity(file_size));
        DraftRecorder {
            sender,
            writer: tokio::spawn(write_draft_progress(persist, entry, receiver, uploaded)),
        }
    }

    pub(super) fn reporter(&self) -> impl FnMut(UploadProgress) + Send + 'static {
        let sender = self.sender.clone();
        move |progress| {
            if let Err(error) = sender.try_send(draft_progress(progress)) {
                tracing::warn!(%error, "failed to queue upload progress");
            }
        }
    }

    pub(super) async fn finish(self) {
        drop(self.sender);
        if let Err(error) = self.writer.await {
            tracing::warn!(%error, "draft progress writer failed");
        }
    }
}

async fn write_draft_progress(
    persist: Persist,
    entry: EntryId,
    mut receiver: mpsc::Receiver<DraftProgress>,
    mut uploaded: UploadedBytes,
) {
    let mut batch = Vec::new();
    while receiver.recv_many(&mut batch, MAX_BATCH).await > 0 {
        let progress = std::mem::take(&mut batch);
        match persist
            .run(move |store| {
                store.record_draft_progress(entry, &progress)?;
                Ok(progress)
            })
            .await
        {
            Ok(progress) => uploaded.add(&progress),
            Err(error) => tracing::warn!(%error, "failed to journal upload progress"),
        }
    }
}

fn progress_capacity(file_size: u64) -> usize {
    let blocks = usize::try_from(file_size.div_ceil(BLOCK_SIZE as u64)).unwrap_or(usize::MAX);
    blocks.saturating_add(1)
}

fn draft_progress(progress: UploadProgress) -> DraftProgress {
    match progress {
        UploadProgress::DraftCreated { link, revision } => DraftProgress::Created {
            link: link.as_str().to_owned(),
            revision: revision.as_str().to_owned(),
        },
        UploadProgress::BlockUploaded(block) => DraftProgress::BlockUploaded(DraftBlock {
            index: block.index(),
            hash: block.hash(),
            size: block.size(),
            digest: block.digest(),
        }),
    }
}
