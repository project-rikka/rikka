use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::pin::pin;

use futures_util::future::BoxFuture;
use futures_util::stream::FuturesUnordered;
use futures_util::{FutureExt as _, StreamExt as _, TryStreamExt as _};
use jiff::Timestamp;
use rikka_proton_api::offload;
use rikka_proton_drive::{
    DriveClient, Error as DriveError, File, Folder, LinkId, Node, ResumePoint, RevisionId,
    ShareKeys, UploadedBlock, VolumeId,
};
use rikka_proton_drive_sync_state::{
    Changes, Entry, EntryId, Kind, LocalItem, Operation, RemoteItem, SyncedItem, UploadDraft,
};
use rikka_proton_pgp::Sha1;
use tokio::io::AsyncWriteExt as _;
use tokio::sync::broadcast;

use crate::engine::drafting::{DraftRecorder, UploadedBytes};
use crate::engine::effect::{Effect, Outcome, SkipReason};
use crate::engine::{Engine, direction_of};
use crate::error::{Error, Result};
use crate::local::files::{TempFile, modified, temp_file_path};
use crate::local::placement::{DownloadedFile, Placement};
use crate::persist::Persist;
use crate::remote::tree::remote_item;
use crate::status::{ItemState, ItemUpdate};

enum SyncedRow {
    New(i64),
    Existing(SyncedItem),
}

pub(super) enum Job {
    Upload(Box<Upload>),
    Download(Box<Download>),
}

pub(super) enum Prepared {
    Ready(Job),
    Skipped(Box<Outcome>),
}

impl Prepared {
    fn skip(reason: SkipReason) -> Result<Prepared> {
        Prepared::skipped(Outcome::skipped(reason))
    }

    fn skipped(outcome: Outcome) -> Result<Prepared> {
        Ok(Prepared::Skipped(Box::new(outcome)))
    }
}

pub(super) struct Upload {
    client: DriveClient,
    share: ShareKeys,
    volume: VolumeId,
    folder: Folder,
    existing_file: Option<File>,
    path: PathBuf,
    name: String,
    local: LocalItem,
    sha1: String,
    synced_row: SyncedRow,
    journal: TransferJournal,
}

struct TransferJournal {
    persist: Persist,
    entry: EntryId,
    resume: Option<ResumePoint>,
    items: broadcast::Sender<ItemUpdate>,
}

pub(super) struct Download {
    client: DriveClient,
    volume: VolumeId,
    file: File,
    remote: RemoteItem,
    folder: PathBuf,
    name: String,
    parent: u64,
    temporary: PathBuf,
    replacing: Option<LocalItem>,
    synced_row: SyncedRow,
}

impl Engine {
    pub(super) async fn transfer(&mut self, mut work: Vec<(usize, Entry)>) {
        work.sort_by_key(|(index, entry)| self.roots[*index].operation_size(&entry.operation));
        let mut queue = work.into_iter();
        let mut running = FuturesUnordered::new();
        let mut inflight: Vec<EntryId> = Vec::new();
        loop {
            while running.len() < self.transfers && !self.is_interrupted() {
                let Some((index, entry)) = queue.next() else {
                    break;
                };
                let path = self.roots[index].operation_path(&entry.operation);
                let id = entry.id;
                if let Err(error) = self.persist.run(move |store| store.begin(id, None)).await {
                    tracing::warn!(%error, "failed to journal operation start");
                    continue;
                }
                match self.prepare(index, &entry).await {
                    Ok(Prepared::Ready(job)) => {
                        if let Some(direction) = direction_of(&entry.operation) {
                            self.notify_item(path.clone(), ItemState::Syncing(direction));
                        }
                        inflight.push(id);
                        running.push(async move {
                            let outcome = job.run().await;
                            (index, entry, path, outcome)
                        });
                    }
                    Ok(Prepared::Skipped(outcome)) => {
                        self.complete_operation(index, entry, path, Ok(*outcome))
                            .await
                    }
                    Err(error) => {
                        self.complete_operation(index, entry, path, Err(error))
                            .await
                    }
                }
            }
            if running.is_empty() {
                return;
            }
            tokio::select! {
                Some((index, entry, path, outcome)) = running.next() => {
                    inflight.retain(|inflight_id| *inflight_id != entry.id);
                    self.complete_operation(index, entry, path, outcome).await;
                }
                command = self.channels.commands.recv() => match command {
                    Some(command) => self.command(command),
                    None => self.closed = true,
                },
                Ok(()) = self.channels.pause.changed() => {
                    self.paused = *self.channels.pause.borrow_and_update();
                }
            }
            if self.paused || self.shutdown_reply.is_some() || self.closed {
                drop(running);
                for id in inflight {
                    if let Err(error) = self.persist.run(move |store| store.release(id)).await {
                        tracing::warn!(%error, "failed to release journaled transfer");
                    }
                }
                return;
            }
        }
    }

    pub(super) async fn prepare(&mut self, index: usize, entry: &Entry) -> Result<Prepared> {
        if self.roots[index].remote.is_none() {
            return Prepared::skip(SkipReason::RemoteFolderMissing);
        }
        match &entry.operation {
            Operation::Upload {
                local,
                parent,
                name,
                sha1,
            } => {
                let journal = self.transfer_journal(entry).await?;
                self.prepare_upload(index, *local, parent, name, sha1, journal)
                    .await
            }
            Operation::UploadRevision { synced, sha1 } => {
                let journal = self.transfer_journal(entry).await?;
                self.prepare_revision_upload(index, *synced, sha1, journal)
                    .await
            }
            Operation::Download {
                remote,
                parent,
                name,
                revision,
            } => {
                self.prepare_download(index, entry.id, remote, *parent, name, revision)
                    .await
            }
            Operation::DownloadRevision { synced, revision } => {
                self.prepare_revision_download(index, entry.id, *synced, revision)
                    .await
            }
            _ => Prepared::skip(SkipReason::NotATransfer),
        }
    }

    async fn transfer_journal(&self, entry: &Entry) -> Result<TransferJournal> {
        let id = entry.id;
        let resume = self
            .persist
            .run(move |store| store.draft(id))
            .await?
            .map(resume_point);
        Ok(TransferJournal {
            persist: self.persist.clone(),
            entry: id,
            resume,
            items: self.channels.items.clone(),
        })
    }

    async fn prepare_upload(
        &mut self,
        index: usize,
        local: u64,
        parent: &str,
        name: &str,
        sha1: &str,
        journal: TransferJournal,
    ) -> Result<Prepared> {
        let client = self.client.clone();
        let root = &mut self.roots[index];
        let (Some(item), Some(path)) = (
            root.state.local().get(&local).cloned(),
            root.local_path(local),
        ) else {
            return Prepared::skip(SkipReason::ItemMissing);
        };
        let Some(remote) = root.remote.as_mut() else {
            return Prepared::skip(SkipReason::RemoteFolderMissing);
        };
        let folder = remote.folder(&client, &root.state, parent).await?;
        let synced_row = SyncedRow::New(root.state.allocate_synced_id());
        Ok(Prepared::Ready(Job::Upload(Box::new(Upload {
            client,
            share: remote.share().clone(),
            volume: remote.volume().clone(),
            folder,
            existing_file: None,
            path,
            name: name.to_owned(),
            local: item,
            sha1: sha1.to_owned(),
            synced_row,
            journal,
        }))))
    }

    async fn prepare_revision_upload(
        &mut self,
        index: usize,
        synced: i64,
        sha1: &str,
        journal: TransferJournal,
    ) -> Result<Prepared> {
        let client = self.client.clone();
        let root = &mut self.roots[index];
        let Some(row) = root.state.synced().get(&synced).cloned() else {
            return Prepared::skip(SkipReason::SyncedItemMissing);
        };
        let (Some(item), Some(path), Some(recorded_item)) = (
            root.state.local().get(&row.local).cloned(),
            root.local_path(row.local),
            root.state.remote().get(&row.remote),
        ) else {
            return Prepared::skip(SkipReason::SyncSideMissing);
        };
        if recorded_item.revision != row.revision {
            return Prepared::skipped(Outcome::remote_changed());
        }
        let Some(remote) = root.remote.as_mut() else {
            return Prepared::skip(SkipReason::RemoteFolderMissing);
        };
        let (folder, file) = remote.file(&client, &root.state, &row.remote).await?;
        let active_revision = file
            .active_revision()
            .map(|revision| revision.id().as_str());
        if active_revision != row.revision.as_deref() {
            return Prepared::skipped(Outcome::remote_changed());
        }
        Ok(Prepared::Ready(Job::Upload(Box::new(Upload {
            client,
            share: remote.share().clone(),
            volume: remote.volume().clone(),
            folder,
            existing_file: Some(file),
            path,
            name: item.name.clone(),
            local: item,
            sha1: sha1.to_owned(),
            synced_row: SyncedRow::Existing(row),
            journal,
        }))))
    }

    async fn prepare_download(
        &mut self,
        index: usize,
        entry: EntryId,
        remote_id: &str,
        parent: u64,
        name: &str,
        revision: &str,
    ) -> Result<Prepared> {
        let client = self.client.clone();
        let root = &mut self.roots[index];
        let (Some(recorded_item), Some(folder)) = (
            root.state.remote().get(remote_id).cloned(),
            root.local_path(parent),
        ) else {
            return Prepared::skip(SkipReason::RemoteItemMissing);
        };
        let Some(remote) = root.remote.as_mut() else {
            return Prepared::skip(SkipReason::RemoteFolderMissing);
        };
        let (_, file) = remote.file(&client, &root.state, remote_id).await?;
        if file.revision_id()?.as_str() != revision {
            return Prepared::skipped(Outcome::remote_changed());
        }
        let temporary = temp_file_path(&folder);
        journal_temporary(self, entry, &temporary).await?;
        let root = &mut self.roots[index];
        let synced_row = SyncedRow::New(root.state.allocate_synced_id());
        Ok(Prepared::Ready(Job::Download(Box::new(Download {
            client,
            volume: VolumeId::from(root.record.volume.as_str()),
            file,
            remote: recorded_item,
            folder,
            name: name.to_owned(),
            parent,
            temporary,
            replacing: None,
            synced_row,
        }))))
    }

    async fn prepare_revision_download(
        &mut self,
        index: usize,
        entry: EntryId,
        synced: i64,
        revision: &str,
    ) -> Result<Prepared> {
        let client = self.client.clone();
        let root = &mut self.roots[index];
        let Some(row) = root.state.synced().get(&synced).cloned() else {
            return Prepared::skip(SkipReason::SyncedItemMissing);
        };
        let (Some(item), Some(recorded_item), Some(target)) = (
            root.state.local().get(&row.local).cloned(),
            root.state.remote().get(&row.remote).cloned(),
            root.local_path(row.local),
        ) else {
            return Prepared::skip(SkipReason::SyncSideMissing);
        };
        let Some(folder) = target.parent().map(Path::to_owned) else {
            return Prepared::skip(SkipReason::NoParentFolder);
        };
        if item.sha1 != row.sha1 {
            return Prepared::skipped(Outcome::invalidate_local(
                SkipReason::ChangedLocally,
                Some(folder),
            ));
        }
        let Some(remote) = root.remote.as_mut() else {
            return Prepared::skip(SkipReason::RemoteFolderMissing);
        };
        let (_, file) = remote.file(&client, &root.state, &row.remote).await?;
        if file.revision_id()?.as_str() != revision {
            return Prepared::skipped(Outcome::remote_changed());
        }
        let temporary = temp_file_path(&folder);
        journal_temporary(self, entry, &temporary).await?;
        let root = &self.roots[index];
        Ok(Prepared::Ready(Job::Download(Box::new(Download {
            client,
            volume: VolumeId::from(root.record.volume.as_str()),
            file,
            remote: recorded_item,
            folder,
            name: item.name.clone(),
            parent: item.parent,
            temporary,
            replacing: Some(item),
            synced_row: SyncedRow::Existing(row),
        }))))
    }
}

async fn journal_temporary(engine: &Engine, entry: EntryId, temporary: &Path) -> Result<()> {
    let artifact = temporary.to_owned();
    engine
        .persist
        .run(move |store| store.record_artifact(entry, &artifact))
        .await
}

impl Job {
    pub(super) fn run(self) -> BoxFuture<'static, Result<Outcome>> {
        match self {
            Job::Upload(upload) => upload.run().boxed(),
            Job::Download(download) => download.run().boxed(),
        }
    }
}

impl Upload {
    async fn run(self) -> Result<Outcome> {
        let folder_path = self.path.parent().map(Path::to_owned);
        let metadata = tokio::fs::symlink_metadata(&self.path)
            .await
            .map_err(Error::io(&self.path))?;
        if metadata.len() != self.local.size || modified(&metadata) != self.local.modified {
            return Ok(Outcome::invalidate_local(
                SkipReason::ChangedBeforeUpload,
                folder_path,
            ));
        }
        let modified_at = Timestamp::from_nanosecond(i128::from(self.local.modified))
            .unwrap_or_else(|_| Timestamp::now());
        let uploaded_bytes = UploadedBytes::new(
            self.journal.items.clone(),
            self.path.clone(),
            self.journal.resume.as_ref(),
        );
        let recorder = DraftRecorder::start(
            self.journal.persist.clone(),
            self.journal.entry,
            self.local.size,
            uploaded_bytes,
        );
        let resume = self.journal.resume.clone();
        let resumed = resume.is_some();
        let uploaded = match &self.existing_file {
            Some(file) => {
                self.with_journal(
                    &recorder,
                    self.client.upload_revision(&self.share, &self.folder, file),
                    resume,
                )
                .modified_at(modified_at)
                .expected_sha1(&self.sha1)
                .upload_file(&self.path)
                .await
            }
            None => match self.send(&recorder, &self.name, modified_at, resume).await {
                Err(DriveError::Conflict { .. }) => {
                    let available = self
                        .client
                        .available_name(&self.volume, &self.folder, &self.name)
                        .await?;
                    self.send(&recorder, &available, modified_at, None).await
                }
                uploaded => uploaded,
            },
        };
        recorder.finish().await;
        if resumed
            && matches!(
                uploaded,
                Err(DriveError::Api(rikka_proton_api::Error::Api(_)))
            )
        {
            let entry = self.journal.entry;
            self.journal
                .persist
                .run(move |store| store.delete_draft(entry))
                .await?;
        }
        let file = match uploaded {
            Ok(file) => file,
            Err(DriveError::ChecksumMismatch { .. }) => {
                return Ok(Outcome::invalidate_local(
                    SkipReason::ChangedDuringUpload,
                    folder_path,
                ));
            }
            Err(error) => return Err(error.into()),
        };
        let Some(item) = remote_item(&Node::File(file)) else {
            return Ok(Outcome::skipped(SkipReason::UploadedFileUnreadable));
        };
        let row = match self.synced_row {
            SyncedRow::New(id) => SyncedItem::from_sides(id, &self.local, &item),
            SyncedRow::Existing(row) => SyncedItem {
                sha1: item.sha1.clone().or(Some(self.sha1)),
                size: self.local.size,
                revision: item.revision.clone(),
                ..row
            },
        };
        Ok(Outcome::Done(
            Effect::new(Changes {
                remote: vec![item],
                synced: vec![row],
                ..Changes::default()
            })
            .with_path(self.path),
        ))
    }

    async fn send(
        &self,
        recorder: &DraftRecorder,
        name: &str,
        modified_at: Timestamp,
        resume: Option<ResumePoint>,
    ) -> rikka_proton_drive::Result<File> {
        self.with_journal(
            recorder,
            self.client.upload(&self.share, &self.folder, name),
            resume,
        )
        .modified_at(modified_at)
        .expected_sha1(&self.sha1)
        .upload_file(&self.path)
        .await
    }

    fn with_journal<'u>(
        &self,
        recorder: &DraftRecorder,
        upload: rikka_proton_drive::Upload<'u>,
        resume: Option<ResumePoint>,
    ) -> rikka_proton_drive::Upload<'u> {
        let upload = upload.on_progress(recorder.reporter());
        match resume {
            Some(point) => upload.resume_from(point),
            None => upload,
        }
    }
}

fn resume_point(draft: UploadDraft) -> ResumePoint {
    ResumePoint::new(
        LinkId::from(draft.link),
        RevisionId::from(draft.revision),
        draft
            .blocks
            .into_iter()
            .map(|block| UploadedBlock::new(block.index, block.hash, block.size, block.digest))
            .collect(),
    )
}

impl Download {
    async fn run(self) -> Result<Outcome> {
        let temporary = TempFile::new(self.temporary.clone());
        let target = self.folder.join(&self.name);
        let sha1 = self.fetch(temporary.path()).await?;
        if self
            .remote
            .sha1
            .as_ref()
            .is_some_and(|expected| *expected != sha1)
        {
            return Err(Error::Corrupted(target));
        }

        let download = DownloadedFile {
            temporary,
            folder: self.folder.clone(),
            name: self.name.clone(),
            modified: self.remote.modified,
        };
        let replacing = self.replacing.clone();
        let placement_result = offload(move || download.place(replacing.as_ref()))
            .await
            .map_err(Error::io(&target))?;
        let (path, metadata) = match placement_result {
            Placement::Placed { path, metadata } => (path, metadata),
            Placement::ChangedLocally => {
                return Ok(Outcome::invalidate_local(
                    SkipReason::ChangedLocally,
                    Some(self.folder.clone()),
                ));
            }
        };
        let removed = self.replacing.as_ref().map(|expected| expected.id);
        let item = LocalItem {
            id: metadata.ino(),
            parent: self.parent,
            name: path
                .file_name()
                .and_then(|name| name.to_str())
                .map_or_else(|| self.name.clone(), str::to_owned),
            kind: Kind::File,
            size: metadata.len(),
            modified: modified(&metadata),
            sha1: Some(sha1.clone()),
        };
        let row = match self.synced_row {
            SyncedRow::New(id) => SyncedItem::from_sides(id, &item, &self.remote),
            SyncedRow::Existing(row) => SyncedItem {
                local: item.id,
                sha1: self.remote.sha1.clone().or(Some(sha1)),
                size: item.size,
                revision: self.remote.revision.clone(),
                ..row
            },
        };
        Ok(Outcome::Done(
            Effect::new(Changes {
                local: vec![item],
                local_removed: removed
                    .filter(|old| *old != metadata.ino())
                    .into_iter()
                    .collect(),
                synced: vec![row],
                ..Changes::default()
            })
            .with_path(path),
        ))
    }

    async fn fetch(&self, temporary: &Path) -> Result<String> {
        let mut output = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temporary)
            .await
            .map_err(Error::io(temporary))?;
        let mut blocks = pin!(self.client.download(&self.volume, &self.file).into_stream());
        let mut digest = Sha1::new();
        while let Some(block) = blocks.try_next().await? {
            digest.update(&block);
            output
                .write_all(&block)
                .await
                .map_err(Error::io(temporary))?;
        }
        output.flush().await.map_err(Error::io(temporary))?;
        output.sync_all().await.map_err(Error::io(temporary))?;
        Ok(rikka_types::hex::encode(digest.finalize()))
    }
}
