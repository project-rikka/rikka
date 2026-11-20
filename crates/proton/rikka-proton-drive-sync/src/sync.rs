use std::path::PathBuf;

use rikka_proton_drive::{DeviceId, DriveClient};
use rikka_proton_drive_sync_state::Target;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::task::JoinHandle;

use crate::engine::{Channels, Command, Engine, Settings};
use crate::error::{Error, Result};
use crate::feed::{FeedBatch, VolumeFeed};
use crate::local::watch::LocalWatcher;
use crate::setup::set_up;
use crate::status::{ItemUpdate, Status};

const DEFAULT_TRANSFERS: usize = 4;
const ITEM_UPDATE_CAPACITY: usize = 1024;
const COMMAND_CAPACITY: usize = 4;

#[must_use]
#[derive(Debug)]
pub struct DriveSyncBuilder {
    client: DriveClient,
    state_folder: Option<PathBuf>,
    device_name: Option<String>,
    folders: Vec<(PathBuf, Target)>,
    transfers: usize,
    feed: Option<broadcast::Receiver<FeedBatch>>,
}

impl DriveSyncBuilder {
    pub fn state_folder(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_folder = Some(path.into());
        self
    }

    pub fn device_name(mut self, name: impl Into<String>) -> Self {
        self.device_name = Some(name.into());
        self
    }

    pub fn computer_folder(mut self, path: impl Into<PathBuf>) -> Self {
        self.folders.push((path.into(), Target::Computer));
        self
    }

    pub fn my_files_folder(mut self, path: impl Into<PathBuf>, remote: impl Into<String>) -> Self {
        self.folders
            .push((path.into(), Target::MyFiles(remote.into())));
        self
    }

    pub fn transfers(mut self, count: usize) -> Self {
        self.transfers = count.max(1);
        self
    }

    pub fn with_feed(mut self, feed: &VolumeFeed) -> Self {
        self.feed = Some(feed.subscribe());
        self
    }

    pub async fn start(self) -> Result<DriveSync> {
        let state_folder = self.state_folder.ok_or(Error::NoStateFolder)?;
        let device = self.device_name.unwrap_or_else(host_name);
        let setup = set_up(self.client, &state_folder, &device, self.folders).await?;
        let paths: Vec<PathBuf> = setup
            .roots
            .iter()
            .map(|root| root.path().to_owned())
            .collect();
        let watcher = LocalWatcher::start(&paths)?;
        let excluded = vec![
            state_folder
                .canonicalize()
                .map_err(Error::io(&state_folder))?,
        ];

        let (commands, command_receiver) = mpsc::channel(COMMAND_CAPACITY);
        let (paused, pause) = watch::channel(false);
        let (status, status_receiver) = watch::channel(Status::starting());
        let (items, _) = broadcast::channel(ITEM_UPDATE_CAPACITY);
        let engine = Engine::new(
            setup.client,
            setup.persist,
            setup.roots,
            Settings {
                device,
                device_id: setup.device.clone(),
                transfers: self.transfers,
                excluded,
            },
            watcher,
            Channels {
                commands: command_receiver,
                pause,
                status,
                items: items.clone(),
                feed: self.feed,
            },
        );
        let task = tokio::spawn(engine.run());

        Ok(DriveSync {
            commands,
            paused,
            status: status_receiver,
            items,
            task,
            device: setup.device,
        })
    }
}

#[derive(Debug)]
pub struct DriveSync {
    commands: mpsc::Sender<Command>,
    paused: watch::Sender<bool>,
    status: watch::Receiver<Status>,
    items: broadcast::Sender<ItemUpdate>,
    task: JoinHandle<()>,
    device: Option<DeviceId>,
}

impl DriveSync {
    pub fn builder(client: DriveClient) -> DriveSyncBuilder {
        DriveSyncBuilder {
            client,
            state_folder: None,
            device_name: None,
            folders: Vec::new(),
            transfers: DEFAULT_TRANSFERS,
            feed: None,
        }
    }

    #[must_use]
    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    #[must_use]
    pub fn watch(&self) -> watch::Receiver<Status> {
        self.status.clone()
    }

    #[must_use]
    pub fn items(&self) -> broadcast::Receiver<ItemUpdate> {
        self.items.subscribe()
    }

    #[must_use]
    pub fn device(&self) -> Option<&DeviceId> {
        self.device.as_ref()
    }

    pub fn pause(&self) {
        self.paused.send_replace(true);
    }

    pub fn resume(&self) {
        self.paused.send_replace(false);
    }

    pub async fn sync_now(&self) -> Result<()> {
        let (reply, done) = oneshot::channel();
        self.commands
            .send(Command::SyncNow(reply))
            .await
            .map_err(|_| Error::Stopped)?;
        done.await.map_err(|_| Error::Stopped)?
    }

    pub async fn shutdown(self) -> Result<()> {
        let (reply, done) = oneshot::channel();
        if self.commands.send(Command::Shutdown(reply)).await.is_ok() {
            drop(done.await);
        }
        self.task.await.map_err(|_| Error::Stopped)
    }
}

fn host_name() -> String {
    let name = rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .into_owned();
    match name.trim() {
        "" => "Linux".to_owned(),
        trimmed => trimmed.to_owned(),
    }
}
