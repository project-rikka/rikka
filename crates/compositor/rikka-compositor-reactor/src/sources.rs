use std::os::fd::{AsRawFd, RawFd};
use std::sync::Arc;
use std::time::Instant;

use crate::interest::Ready;
use crate::mailbox::Notifier;

pub(crate) type WatchFn<S> = Box<dyn FnMut(&mut S, Ready)>;
pub(crate) type DrainFn<S> = Box<dyn FnMut(&mut S)>;
pub(crate) type TimerFn<S> = Box<dyn FnOnce(&mut S)>;

pub(crate) enum Source<S> {
    Watch {
        fd: RawFd,
        callback: Option<WatchFn<S>>,
    },
    Mailbox {
        notifier: Arc<Notifier>,
        drain: Option<DrainFn<S>>,
    },
    EachTurn {
        callback: Option<DrainFn<S>>,
    },
    Task {
        callback: Option<DrainFn<S>>,
        woken: bool,
    },
    Timer {
        at: Instant,
        callback: TimerFn<S>,
    },
}

pub(crate) enum TakenCallback<S> {
    Watch(WatchFn<S>),
    Mailbox(Arc<Notifier>, DrainFn<S>),
}

struct Slot<S> {
    generation: u32,
    source: Option<Source<S>>,
}

pub(crate) struct Sources<S> {
    slots: Vec<Slot<S>>,
    free: Vec<u32>,
    turn_order: Vec<u64>,
}

impl<S> Sources<S> {
    pub(crate) fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            turn_order: Vec::new(),
        }
    }

    pub(crate) fn insert(&mut self, source: Source<S>) -> u64 {
        let each_turn = matches!(source, Source::EachTurn { .. });
        let index = match self.free.pop() {
            Some(index) => {
                self.slots[index as usize].source = Some(source);
                index
            }
            None => {
                self.slots.push(Slot {
                    generation: 1,
                    source: Some(source),
                });
                (self.slots.len() - 1) as u32
            }
        };
        let token = token(index, self.slots[index as usize].generation);
        if each_turn {
            self.turn_order.push(token);
        }
        token
    }

    pub(crate) fn remove(&mut self, token: u64) -> Option<Source<S>> {
        let (index, generation) = split(token);
        let slot = self.slots.get_mut(index as usize)?;
        if slot.generation != generation {
            return None;
        }
        let removed = slot.source.take()?;
        slot.generation = slot.generation.wrapping_add(1).max(1);
        self.free.push(index);
        if matches!(removed, Source::EachTurn { .. }) {
            self.turn_order.retain(|each| *each != token);
        }
        Some(removed)
    }

    pub(crate) fn watches_fd(&self, fd: RawFd) -> bool {
        self.slots.iter().any(|slot| {
            matches!(&slot.source, Some(Source::Watch { fd: watched, .. }) if *watched == fd)
        })
    }

    pub(crate) fn fd_of(&self, token: u64) -> Option<RawFd> {
        match self.get(token)? {
            Source::Watch { fd, .. } => Some(*fd),
            Source::Mailbox { notifier, .. } => Some(notifier.fd().as_raw_fd()),
            _ => None,
        }
    }

    pub(crate) fn take(&mut self, token: u64) -> Option<TakenCallback<S>> {
        match self.get_mut(token)? {
            Source::Watch { callback, .. } => callback.take().map(TakenCallback::Watch),
            Source::Mailbox { notifier, drain } => {
                let notifier = notifier.clone();
                drain
                    .take()
                    .map(|drain| TakenCallback::Mailbox(notifier, drain))
            }
            _ => None,
        }
    }

    pub(crate) fn restore(&mut self, token: u64, callback_entry: TakenCallback<S>) {
        match (self.get_mut(token), callback_entry) {
            (Some(Source::Watch { callback, .. }), TakenCallback::Watch(returned)) => {
                *callback = Some(returned);
            }
            (Some(Source::Mailbox { drain, .. }), TakenCallback::Mailbox(_, returned)) => {
                *drain = Some(returned);
            }
            _ => {}
        }
    }

    pub(crate) fn turn_hooks(&self, into: &mut Vec<u64>) {
        into.clear();
        into.extend_from_slice(&self.turn_order);
    }

    pub(crate) fn take_hook(&mut self, token: u64) -> Option<DrainFn<S>> {
        match self.get_mut(token)? {
            Source::EachTurn { callback } => callback.take(),
            Source::Task { callback, woken } => {
                *woken = false;
                callback.take()
            }
            _ => None,
        }
    }

    pub(crate) fn restore_hook(&mut self, token: u64, returned: DrainFn<S>) {
        if let Some(Source::EachTurn { callback } | Source::Task { callback, .. }) =
            self.get_mut(token)
        {
            *callback = Some(returned);
        }
    }

    pub(crate) fn wake(&mut self, token: u64) -> bool {
        match self.get_mut(token) {
            Some(Source::Task { woken, .. }) if !*woken => {
                *woken = true;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn timer_at(&self, token: u64) -> Option<Instant> {
        match self.get(token)? {
            Source::Timer { at, .. } => Some(*at),
            _ => None,
        }
    }

    pub(crate) fn take_timer(&mut self, token: u64) -> Option<TimerFn<S>> {
        match self.remove(token)? {
            Source::Timer { callback, .. } => Some(callback),
            _ => None,
        }
    }

    fn get(&self, token: u64) -> Option<&Source<S>> {
        let (index, generation) = split(token);
        let slot = self.slots.get(index as usize)?;
        (slot.generation == generation)
            .then_some(slot.source.as_ref())
            .flatten()
    }

    fn get_mut(&mut self, token: u64) -> Option<&mut Source<S>> {
        let (index, generation) = split(token);
        let slot = self.slots.get_mut(index as usize)?;
        (slot.generation == generation)
            .then_some(slot.source.as_mut())
            .flatten()
    }
}

fn token(index: u32, generation: u32) -> u64 {
    (u64::from(generation) << 32) | u64::from(index)
}

fn split(token: u64) -> (u32, u32) {
    (token as u32, (token >> 32) as u32)
}
