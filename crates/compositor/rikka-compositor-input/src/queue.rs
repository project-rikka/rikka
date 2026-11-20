use rikka_sync::Mutex;
use std::fmt;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustix::event::{EventfdFlags, eventfd};

use crate::event::InputEvent;

#[derive(Clone)]
pub struct InputQueue {
    shared: Arc<Shared>,
}

struct Shared {
    events: Mutex<Vec<InputEvent>>,
    armed: AtomicBool,
    wake: OwnedFd,
}

impl InputQueue {
    pub fn new() -> io::Result<Self> {
        let wake = eventfd(0, EventfdFlags::CLOEXEC | EventfdFlags::NONBLOCK)?;
        Ok(Self {
            shared: Arc::new(Shared {
                events: Mutex::new(Vec::new()),
                armed: AtomicBool::new(true),
                wake,
            }),
        })
    }

    pub fn fd(&self) -> BorrowedFd<'_> {
        self.shared.wake.as_fd()
    }

    pub fn drain(&self, into: &mut Vec<InputEvent>) {
        let mut count = [0u8; 8];
        let _ = rustix::io::read(&self.shared.wake, &mut count);
        self.shared.armed.store(true, Ordering::SeqCst);
        let mut events = self.shared.events.lock();
        if into.is_empty() {
            std::mem::swap(into, &mut events);
        } else {
            into.append(&mut events);
        }
    }

    pub(crate) fn push(&self, events: &mut Vec<InputEvent>) {
        if events.is_empty() {
            return;
        }
        self.shared.events.lock().append(events);
        if self.shared.armed.swap(false, Ordering::SeqCst) {
            let _ = rustix::io::write(&self.shared.wake, &1u64.to_ne_bytes());
        }
    }
}

impl fmt::Debug for InputQueue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InputQueue")
            .field("armed", &self.shared.armed.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}
