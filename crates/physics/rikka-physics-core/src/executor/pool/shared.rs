use rikka_sync::Mutex;
use std::any::Any;
use std::cell::UnsafeCell;
use std::mem;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering, fence};
use std::time::Duration;

use super::parking::{Parking, wait_yielding};
use crate::CachePadded;
use crate::executor::{Block, Stage};

const CLOSED: u32 = 1 << 31;
const IDLE_SPIN: Duration = Duration::from_micros(100);
const BARRIER_SPIN: Duration = Duration::from_micros(500);
const CALLER_SPIN: Duration = Duration::from_micros(500);

type Payload = Box<dyn Any + Send>;

#[derive(Default)]
pub(super) struct StageSync {
    claimed: CachePadded<AtomicU32>,
    finished: CachePadded<AtomicU32>,
}

#[derive(Clone, Copy)]
struct Job {
    stages: *const [Stage],
    syncs: *const [StageSync],
    task: *const (dyn Fn(Block) + Sync),
    last: usize,
}

pub(super) struct Shared {
    epoch: CachePadded<AtomicU64>,
    entered: CachePadded<AtomicU32>,
    job: UnsafeCell<Option<Job>>,
    aborted: AtomicBool,
    shutdown: AtomicBool,
    panic: Mutex<Option<Payload>>,
    parking: Parking,
}

unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

impl Shared {
    pub(super) fn new(workers: usize) -> Self {
        Self {
            epoch: CachePadded::default(),
            entered: CachePadded::new(AtomicU32::new(CLOSED)),
            job: UnsafeCell::new(None),
            aborted: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            panic: Mutex::new(None),
            parking: Parking::new(workers),
        }
    }

    pub(super) fn run<'a>(
        &self,
        syncs: &[StageSync],
        stages: &[Stage],
        task: &'a (dyn Fn(Block) + Sync + 'a),
    ) -> Option<Payload> {
        let last = stages
            .iter()
            .rposition(|stage| stage.blocks > 0)
            .unwrap_or(0);
        for (position, (stage, sync)) in stages.iter().zip(syncs).enumerate() {
            if stage.blocks > 1 {
                sync.claimed.store(0, Ordering::Relaxed);
            }
            if position < last && stage.blocks > 0 {
                sync.finished.store(0, Ordering::Relaxed);
            }
        }
        let erased: *const (dyn Fn(Block) + Sync + 'a) = task;
        let job = Job {
            stages,
            syncs,
            task: unsafe {
                mem::transmute::<
                    *const (dyn Fn(Block) + Sync + 'a),
                    *const (dyn Fn(Block) + Sync + 'static),
                >(erased)
            },
            last,
        };
        unsafe { *self.job.get() = Some(job) };
        self.aborted.store(false, Ordering::Relaxed);
        self.entered.fetch_and(!CLOSED, Ordering::Release);
        self.epoch.fetch_add(1, Ordering::SeqCst);
        self.parking.wake_all();

        self.work(stages, syncs, task, 0, last);
        self.entered.fetch_or(CLOSED, Ordering::SeqCst);
        wait_yielding(CALLER_SPIN, || {
            self.entered.load(Ordering::Acquire) & !CLOSED == 0
        });
        if !self.aborted.load(Ordering::Relaxed) {
            return None;
        }
        self.panic.lock().take()
    }

    pub(super) fn serve(&self, worker: u32) {
        self.parking.register(worker);
        let mut last_epoch = 0;
        loop {
            self.parking.wait(worker, IDLE_SPIN, || {
                self.epoch.load(Ordering::SeqCst) != last_epoch
                    || self.shutdown.load(Ordering::SeqCst)
            });
            if self.shutdown.load(Ordering::SeqCst) {
                return;
            }
            last_epoch = self.epoch.load(Ordering::SeqCst);
            self.join(worker);
        }
    }

    pub(super) fn shut_down(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.parking.wake_all();
    }

    fn join(&self, worker: u32) {
        if self.entered.fetch_add(1, Ordering::Acquire) & CLOSED == 0
            && let Some(job) = unsafe { *self.job.get() }
        {
            let (stages, syncs, task) = unsafe { (&*job.stages, &*job.syncs, &*job.task) };
            self.work(stages, syncs, task, worker, job.last);
        }
        self.entered.fetch_sub(1, Ordering::Release);
    }

    fn work(
        &self,
        stages: &[Stage],
        syncs: &[StageSync],
        task: &(dyn Fn(Block) + Sync),
        worker: u32,
        last: usize,
    ) {
        for ((stage, sync), position) in stages.iter().zip(syncs).zip(0u32..) {
            let blocks = stage.blocks;
            let barrier = (position as usize) < last;
            let last_done = match blocks {
                0 => continue,
                1 if worker == 0 => {
                    self.execute(task, position, 0, worker);
                    barrier && self.finish(sync, 1, 1)
                }
                1 => false,
                _ => {
                    let mut done = 0u32;
                    loop {
                        let index = sync.claimed.fetch_add(1, Ordering::Relaxed);
                        if index >= blocks {
                            break;
                        }
                        self.execute(task, position, index, worker);
                        done += 1;
                    }
                    barrier && done > 0 && self.finish(sync, done, blocks)
                }
            };
            if !barrier || last_done {
                continue;
            }
            let finished = || sync.finished.load(Ordering::SeqCst) >= blocks;
            if worker == 0 {
                wait_yielding(CALLER_SPIN, finished);
            } else {
                self.parking.wait(worker, BARRIER_SPIN, finished);
            }
        }
    }

    #[inline]
    fn execute(&self, task: &(dyn Fn(Block) + Sync), stage: u32, index: u32, worker: u32) {
        if self.aborted.load(Ordering::Relaxed) {
            return;
        }
        let block = Block {
            stage,
            index,
            worker,
        };
        if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| task(block))) {
            let mut slot = self.panic.lock();
            slot.get_or_insert(payload);
            self.aborted.store(true, Ordering::Relaxed);
        }
    }

    #[inline]
    fn finish(&self, sync: &StageSync, done: u32, blocks: u32) -> bool {
        if sync.finished.fetch_add(done, Ordering::AcqRel) + done == blocks {
            fence(Ordering::SeqCst);
            self.parking.wake_all();
            true
        } else {
            false
        }
    }
}
