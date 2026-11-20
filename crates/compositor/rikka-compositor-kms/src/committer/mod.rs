mod command;
mod commit;
mod decide;
mod lead_time;
mod realtime;
mod screen;
mod sequence;
mod slots;
mod stamp;
mod validated;
mod worker;

use std::fmt;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};

use rikka_compositor_reactor::Sender;

pub(crate) use command::{Command, Hardware, Setup};

use crate::drm_fd::DrmFd;
use crate::error::{Error, Result};
use crate::event::KmsEvent;
use crate::ids::CrtcId;
use slots::CursorSlots;

pub(crate) struct Committer {
    commands: Sender<Command>,
    cursors: Arc<CursorSlots>,
    thread: Option<JoinHandle<()>>,
}

#[derive(Debug, Clone)]
pub struct CursorControl {
    cursors: Arc<CursorSlots>,
}

impl Committer {
    pub(crate) fn spawn(
        index: usize,
        fd: DrmFd,
        hardware: Hardware,
        events: Sender<KmsEvent>,
    ) -> Result<Self> {
        let cursors = Arc::new(
            CursorSlots::new(hardware.crtcs.iter().map(|crtc| crtc.id)).map_err(Error::Thread)?,
        );
        let (ready, started) = mpsc::sync_channel(1);
        let moved = cursors.clone();
        let thread = thread::Builder::new()
            .name(format!("rikka-kms-{index}"))
            .spawn(move || worker::run(fd, hardware, moved, events, ready))
            .map_err(Error::Thread)?;
        let commands = started.recv().map_err(|_| Error::CommitterGone)??;
        Ok(Self {
            commands,
            cursors,
            thread: Some(thread),
        })
    }

    pub(crate) fn send(&self, command: Command) -> Result<()> {
        self.commands
            .send(command)
            .map_err(|_| Error::CommitterGone)
    }

    pub(crate) fn cursor_control(&self) -> CursorControl {
        CursorControl {
            cursors: self.cursors.clone(),
        }
    }
}

impl CursorControl {
    pub fn cursor_at(&self, crtc: CrtcId, x: i32, y: i32) -> Result<()> {
        if !self.cursors.is_open() {
            return Err(Error::CommitterGone);
        }
        self.cursors.move_to(crtc, x, y);
        Ok(())
    }
}

impl Drop for Committer {
    fn drop(&mut self) {
        self.cursors.close();
        let _ = self.commands.send(Command::Stop);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::error!("KMS thread panicked");
        }
    }
}

impl fmt::Debug for Committer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Committer")
            .field("running", &self.commands.is_open())
            .finish()
    }
}
