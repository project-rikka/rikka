use std::hint;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::SeqCst};
use std::thread::{self, Thread};
use std::time::{Duration, Instant};

use crate::CachePadded;

const SPINS_BETWEEN_CLOCK_READS: u32 = 64;

#[derive(Default)]
struct Sleeper {
    thread: OnceLock<Thread>,
    parked: AtomicBool,
}

pub(super) struct Parking {
    sleepers: Box<[CachePadded<Sleeper>]>,
    asleep: CachePadded<AtomicU32>,
}

impl Parking {
    pub(super) fn new(workers: usize) -> Self {
        Self {
            sleepers: (0..workers).map(|_| CachePadded::default()).collect(),
            asleep: CachePadded::default(),
        }
    }

    pub(super) fn register(&self, worker: u32) {
        self.sleepers[worker as usize]
            .thread
            .set(thread::current())
            .expect("worker registers once");
    }

    pub(super) fn wait(&self, worker: u32, spin: Duration, ready: impl Fn() -> bool) {
        if spin_until(spin, &ready) {
            return;
        }
        let sleeper = &self.sleepers[worker as usize];
        loop {
            sleeper.parked.store(true, SeqCst);
            self.asleep.fetch_add(1, SeqCst);
            let done = ready();
            if !done {
                thread::park();
            }
            sleeper.parked.store(false, SeqCst);
            self.asleep.fetch_sub(1, SeqCst);
            if done || ready() {
                return;
            }
        }
    }

    #[inline]
    pub(super) fn wake_all(&self) {
        if self.asleep.load(SeqCst) != 0 {
            self.unpark_parked();
        }
    }

    #[cold]
    fn unpark_parked(&self) {
        for sleeper in &self.sleepers {
            if sleeper.parked.swap(false, SeqCst)
                && let Some(thread) = sleeper.thread.get()
            {
                thread.unpark();
            }
        }
    }
}

pub(super) fn spin_until(budget: Duration, ready: impl Fn() -> bool) -> bool {
    let mut start = None;
    loop {
        for _ in 0..SPINS_BETWEEN_CLOCK_READS {
            if ready() {
                return true;
            }
            hint::spin_loop();
        }
        if start.get_or_insert_with(Instant::now).elapsed() >= budget {
            return false;
        }
    }
}

pub(super) fn wait_yielding(spin: Duration, ready: impl Fn() -> bool) {
    if spin_until(spin, &ready) {
        return;
    }
    while !ready() {
        thread::yield_now();
    }
}
