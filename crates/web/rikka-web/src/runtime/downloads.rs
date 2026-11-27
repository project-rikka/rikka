use std::sync::Weak;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use serde_json::json;

use crate::api::{DownloadItem, DownloadState};
use crate::channel::decode;
use crate::event::RuntimeEvent;
use crate::runtime::core::Core;

const PROGRESS_REFRESH: Duration = Duration::from_millis(500);

const LISTED: u32 = 200;

#[derive(Debug)]
pub(crate) struct Poller {
    kick: mpsc::Sender<()>,
}

pub(crate) struct Kicks(mpsc::Receiver<()>);

impl Poller {
    pub(crate) fn new() -> (Poller, Kicks) {
        let (kick, kicks) = mpsc::channel();
        (Poller { kick }, Kicks(kicks))
    }

    pub(crate) fn refresh(&self) {
        if self.kick.send(()).is_err() {
            tracing::debug!("download poller already stopped");
        }
    }
}

impl Kicks {
    pub(crate) fn follow(self, core: Weak<Core>) -> std::io::Result<()> {
        std::thread::Builder::new()
            .name("rikka-web-downloads".into())
            .spawn(move || run(&core, &self.0))?;
        Ok(())
    }
}

fn run(core: &Weak<Core>, kicks: &mpsc::Receiver<()>) {
    let mut active = false;
    loop {
        let woke = match active {
            true => kicks.recv_timeout(PROGRESS_REFRESH),
            false => kicks.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        if let Err(RecvTimeoutError::Disconnected) = woke {
            return;
        }
        while kicks.try_recv().is_ok() {}
        let Some(listed) = list(core) else {
            return;
        };
        let Ok(items) = listed.recv() else {
            continue;
        };
        active = items
            .iter()
            .any(|item| item.state == DownloadState::InProgress && !item.paused);
        match core.upgrade() {
            Some(core) => core.outbox.send(RuntimeEvent::Downloads(items)),
            None => return,
        }
    }
}

fn list(core: &Weak<Core>) -> Option<mpsc::Receiver<Vec<DownloadItem>>> {
    let core = core.upgrade()?;
    let (sender, receiver) = mpsc::channel();
    core.link.call_then(
        "downloads.search",
        vec![json!({ "orderBy": ["-startTime"], "limit": LISTED })],
        move |result| match decode::<Vec<DownloadItem>>(result) {
            Ok(items) => {
                if sender.send(items).is_err() {
                    tracing::debug!("download poller already stopped");
                }
            }
            Err(error) => tracing::debug!(%error, "failed to list downloads"),
        },
    );
    Some(receiver)
}
