use rikka_sync::{Condvar, Mutex};
use std::collections::VecDeque;
use std::sync::mpsc::{SyncSender, sync_channel};

use crate::absolute_position::AbsolutePosition;
use crate::error::{Error, Result};

const CAPACITY: usize = 256;

pub(crate) enum Command {
    MoveTo(AbsolutePosition),
    MoveBy { dx: f64, dy: f64 },
    Button { code: u32, pressed: bool },
    Wheel { x120: i32, y120: i32 },
    Continuous { dx: f64, dy: f64 },
    Sync(SyncSender<()>),
}

#[derive(Default)]
struct Pending {
    commands: VecDeque<Command>,
    shutdown: bool,
}

#[derive(Default)]
pub(crate) struct Queue {
    pending: Mutex<Pending>,
    ready: Condvar,
}

impl Queue {
    pub(crate) fn push(&self, command: Command) -> Result<()> {
        let mut pending = self.pending.lock();
        if pending.shutdown {
            return Err(Error::Stopped);
        }
        if let Some(command) = coalesce(&mut pending.commands, command) {
            if pending.commands.len() >= CAPACITY {
                return Err(Error::QueueFull(CAPACITY));
            }
            pending.commands.push_back(command);
        }
        drop(pending);
        self.ready.notify_one();
        Ok(())
    }

    pub(crate) fn sync(&self) -> Result<()> {
        let (done, flushed) = sync_channel(1);
        self.push(Command::Sync(done))?;
        flushed.recv().map_err(|_| Error::Stopped)
    }

    pub(crate) fn take(&self) -> Option<VecDeque<Command>> {
        let mut pending = self.pending.lock();
        loop {
            if !pending.commands.is_empty() {
                return Some(std::mem::take(&mut pending.commands));
            }
            if pending.shutdown {
                return None;
            }
            pending = self.ready.wait(pending);
        }
    }
}

impl rikka_worker::Stop for Queue {
    fn stop(&self) {
        self.pending.lock().shutdown = true;
        self.ready.notify_all();
    }
}

impl std::fmt::Debug for Queue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Queue").finish_non_exhaustive()
    }
}

fn coalesce(commands: &mut VecDeque<Command>, command: Command) -> Option<Command> {
    match (commands.back_mut(), command) {
        (Some(Command::MoveTo(last)), Command::MoveTo(spot)) => {
            *last = spot;
            None
        }
        (
            Some(Command::MoveBy { dx, dy }),
            Command::MoveBy {
                dx: more_x,
                dy: more_y,
            },
        )
        | (
            Some(Command::Continuous { dx, dy }),
            Command::Continuous {
                dx: more_x,
                dy: more_y,
            },
        ) => {
            *dx += more_x;
            *dy += more_y;
            None
        }
        (
            Some(Command::Wheel { x120, y120 }),
            Command::Wheel {
                x120: more_x,
                y120: more_y,
            },
        ) => {
            *x120 = x120.saturating_add(more_x);
            *y120 = y120.saturating_add(more_y);
            None
        }
        (_, command) => Some(command),
    }
}
