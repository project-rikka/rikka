use std::convert::Infallible;
use std::fs::File;
use std::io::{Seek, SeekFrom};

use rikka_worker::{EventFd, EventNotifier, Stop, Worker};
use rustix::event::{PollFd, PollFlags, poll};

use crate::error::{Error, Result};

const THREAD: &str = "rikka-music-mounts";
const MOUNT_TABLE: &str = "/proc/self/mountinfo";

struct Ending(EventNotifier);

impl Stop for Ending {
    fn stop(&self) {
        if let Err(error) = self.0.notify() {
            tracing::warn!("failed to stop mount watcher: {error}");
        }
    }
}

#[derive(Debug)]
pub(crate) struct Mounts {
    _worker: Worker<Ending>,
}

impl Mounts {
    pub(crate) fn start(changed: impl Fn() + Send + 'static) -> Result<Mounts> {
        let table = File::open(MOUNT_TABLE)
            .map_err(|error| Error::Open(format!("{MOUNT_TABLE}: {error}")))?;
        let (notifier, wake_fd) =
            rikka_worker::event_fd().map_err(|error| Error::Open(format!("{THREAD}: {error}")))?;
        let worker = Worker::spawn(THREAD, Ending(notifier), move || {
            Ok::<_, Infallible>(move || follow(table, &wake_fd, &changed))
        })
        .map_err(|error| Error::Open(format!("{THREAD}: {error}")))?;
        Ok(Mounts { _worker: worker })
    }
}

fn follow(mut table: File, wake_fd: &EventFd, changed: &impl Fn()) {
    if let Err(error) = reread(&mut table) {
        tracing::warn!(%error, "failed to read {MOUNT_TABLE}, no longer watching mounts");
        return;
    }
    loop {
        let mut ready = [
            PollFd::new(&table, PollFlags::PRI | PollFlags::ERR),
            PollFd::new(wake_fd, PollFlags::IN),
        ];
        match poll(&mut ready, None) {
            Ok(_) => {}
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => {
                tracing::warn!(%error, "failed to poll {MOUNT_TABLE}, no longer watching mounts");
                return;
            }
        }
        if !ready[1].revents().is_empty() {
            return;
        }
        if ready[0].revents().is_empty() {
            continue;
        }
        if let Err(error) = reread(&mut table) {
            tracing::warn!(%error, "failed to read {MOUNT_TABLE}, no longer watching mounts");
            return;
        }
        changed();
    }
}

fn reread(table: &mut File) -> std::io::Result<()> {
    table.seek(SeekFrom::Start(0))?;
    std::io::copy(table, &mut std::io::sink())?;
    Ok(())
}
