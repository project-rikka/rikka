use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::time::Duration;

use rikka_input::Chord;

use crate::error::{Error, Result};

const CAPACITY: usize = 256;

pub(crate) enum Command {
    Press { chord: Chord, hold: Duration },
    Keymap(String),
    Modifiers { depressed: u32, locked: u32 },
    Key { code: u32, pressed: bool },
    Text { keymap: String, codes: Vec<u32> },
    Sync(SyncSender<()>),
    Stop,
}

pub(crate) struct Queue(SyncSender<Command>);

impl std::fmt::Debug for Queue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Queue").finish_non_exhaustive()
    }
}

pub(crate) fn channel() -> (Queue, Receiver<Command>) {
    let (sender, commands) = sync_channel(CAPACITY);
    (Queue(sender), commands)
}

impl Queue {
    pub(crate) fn push(&self, command: Command) -> Result<()> {
        self.0.try_send(command).map_err(|error| match error {
            TrySendError::Full(_) => Error::QueueFull(CAPACITY),
            TrySendError::Disconnected(_) => Error::Stopped,
        })
    }

    pub(crate) fn sync(&self) -> Result<()> {
        let (done, flushed) = sync_channel(1);
        self.push(Command::Sync(done))?;
        flushed.recv().map_err(|_| Error::Stopped)
    }
}

impl rikka_worker::Stop for Queue {
    fn stop(&self) {
        if self.0.send(Command::Stop).is_err() {
            tracing::trace!("keyboard worker already stopped");
        }
    }
}
