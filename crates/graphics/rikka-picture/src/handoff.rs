use rikka_sync::{Mutex, MutexGuard};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rikka_types::Size;

use crate::wake::{Sleepers, Waking};

static MINTED: AtomicU64 = AtomicU64::new(0);

static ARRIVALS: Sleepers = Sleepers::new();

pub fn on_arrival(wake: &Waking) {
    ARRIVALS.register(wake);
}

pub fn announce_arrival() {
    ARRIVALS.wake_all();
}

pub(crate) struct Handoff<T> {
    id: u64,
    waiting: Mutex<Option<T>>,
    shape: AtomicU64,
}

impl<T> Handoff<T> {
    pub(crate) fn new() -> Arc<Handoff<T>> {
        Arc::new(Handoff {
            id: MINTED.fetch_add(1, Ordering::Relaxed),
            waiting: Mutex::new(None),
            shape: AtomicU64::new(0),
        })
    }

    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    pub(crate) fn size(&self) -> Option<Size> {
        let shape = self.shape.load(Ordering::Relaxed);
        if shape == 0 {
            return None;
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "picture dimensions stay below 2^24"
        )]
        Some(Size::new(
            (shape >> 32) as u32 as f32,
            (shape & 0xffff_ffff) as u32 as f32,
        ))
    }

    pub(crate) fn give(&self, width: u32, height: u32, picture: T) {
        self.shape.store(
            u64::from(width) << 32 | u64::from(height),
            Ordering::Relaxed,
        );
        let replaced = self.waiting().replace(picture);
        drop(replaced);
        announce_arrival();
    }

    pub(crate) fn give_after(
        &self,
        width: u32,
        height: u32,
        mut picture: T,
        follow: impl FnOnce(&mut T, &mut T),
    ) {
        self.shape.store(
            u64::from(width) << 32 | u64::from(height),
            Ordering::Relaxed,
        );
        let mut waiting = self.waiting();
        if let Some(untaken) = waiting.as_mut() {
            follow(&mut picture, untaken);
        }
        let replaced = waiting.replace(picture);
        drop(waiting);
        drop(replaced);
        announce_arrival();
    }

    pub(crate) fn take(&self) -> Option<T> {
        self.waiting().take()
    }

    pub(crate) fn take_if(&self, ready: impl FnOnce(&T) -> bool) -> Option<T> {
        let mut waiting = self.waiting();
        match waiting.as_ref().is_some_and(ready) {
            true => waiting.take(),
            false => None,
        }
    }

    pub(crate) fn is_waiting(&self) -> bool {
        self.waiting().is_some()
    }

    fn waiting(&self) -> MutexGuard<'_, Option<T>> {
        self.waiting.lock()
    }
}
