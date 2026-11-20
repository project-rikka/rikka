use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::{self, ThreadId};

use rikka_compositor_reactor::Sender;

use crate::error::{Error, Result};

pub(crate) enum DeviceRequest {
    Open {
        path: PathBuf,
        reply: mpsc::SyncSender<Result<OwnedFd>>,
    },
    Close(OwnedFd),
}

#[derive(Debug, Clone)]
pub struct DeviceOpener {
    requests: Sender<DeviceRequest>,
    home: ThreadId,
}

impl DeviceOpener {
    pub(crate) fn new(requests: Sender<DeviceRequest>) -> Self {
        Self {
            requests,
            home: thread::current().id(),
        }
    }

    pub fn open(&self, path: &Path) -> Result<OwnedFd> {
        if thread::current().id() == self.home {
            return Err(Error::OpenedFromSessionThread);
        }
        let (reply, answer) = mpsc::sync_channel(1);
        self.requests
            .send(DeviceRequest::Open {
                path: path.to_owned(),
                reply,
            })
            .map_err(|_| Error::SessionGone)?;
        answer.recv().map_err(|_| Error::SessionGone)?
    }

    pub fn close(&self, fd: OwnedFd) {
        if self.requests.send(DeviceRequest::Close(fd)).is_err() {
            tracing::debug!("device closed after its session ended");
        }
    }
}
