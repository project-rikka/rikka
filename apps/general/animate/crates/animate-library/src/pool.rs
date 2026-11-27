use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::path::Path;
use std::sync::Arc;

use rikka_sync::{Condvar, Mutex};
use rikka_types::CancelToken;
use rikka_worker::{Stop, Worker};

use crate::details::Details;
use crate::entry::{EntryId, Stamp};
use crate::error::{Error, Result};
use crate::format::{Cost, Format};
use crate::probe;

const NAME: &str = "rikka-animate-probe";
const MOST_THREADS: usize = 4;

#[derive(Clone, Debug)]
pub(crate) struct Job {
    pub(crate) id: EntryId,
    pub(crate) path: Arc<Path>,
    pub(crate) format: Format,
    pub(crate) stamp: Stamp,
    pub(crate) depth: usize,
}

pub(crate) struct Probe {
    pub(crate) id: EntryId,
    pub(crate) stamp: Stamp,
    pub(crate) details: Details,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Rank {
    cost: Cost,
    depth: usize,
    size: u64,
    id: EntryId,
}

struct Queued {
    rank: Rank,
    job: Job,
}

impl PartialEq for Queued {
    fn eq(&self, other: &Queued) -> bool {
        self.rank == other.rank
    }
}

impl Eq for Queued {}

impl PartialOrd for Queued {
    fn partial_cmp(&self, other: &Queued) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Queued {
    fn cmp(&self, other: &Queued) -> std::cmp::Ordering {
        self.rank.cmp(&other.rank)
    }
}

#[derive(Default)]
struct Queue {
    heap: BinaryHeap<Reverse<Queued>>,
    wanted: HashMap<EntryId, Stamp>,
    importing: bool,
    closed: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    ready: Condvar,
    cancel: CancelToken,
}

impl Stop for Shared {
    fn stop(&self) {
        self.queue.lock().closed = true;
        self.cancel.cancel();
        self.ready.notify_all();
    }
}

pub(crate) struct Pool {
    shared: Arc<Shared>,
    _workers: Vec<Worker<Arc<Shared>>>,
}

impl Pool {
    pub(crate) fn new(deliver: impl Fn(Probe) + Send + Sync + 'static) -> Result<Pool> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            ready: Condvar::new(),
            cancel: CancelToken::new(),
        });
        let deliver: Arc<dyn Fn(Probe) + Send + Sync> = Arc::new(deliver);
        let threads = std::thread::available_parallelism()
            .map_or(1, |threads| threads.get() / 2)
            .clamp(1, MOST_THREADS);
        let workers = (0..threads)
            .map(|_| {
                let shared = Arc::clone(&shared);
                let deliver = Arc::clone(&deliver);
                Worker::spawn(NAME, Arc::clone(&shared), move || {
                    Ok::<_, Error>(move || work(&shared, deliver.as_ref()))
                })
                .map_err(Error::from)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Pool {
            shared,
            _workers: workers,
        })
    }

    pub(crate) fn want(&self, job: Job) {
        let rank = Rank {
            cost: job.format.cost(),
            depth: job.depth,
            size: job.stamp.size,
            id: job.id,
        };
        let mut queue = self.shared.queue.lock();
        queue.wanted.insert(job.id, job.stamp);
        queue.heap.push(Reverse(Queued { rank, job }));
        drop(queue);
        self.shared.ready.notify_one();
    }

    pub(crate) fn unwant(&self, id: EntryId, stamp: Stamp) {
        let mut queue = self.shared.queue.lock();
        if queue.wanted.get(&id) == Some(&stamp) {
            queue.wanted.remove(&id);
        }
    }

    pub(crate) fn forget(&self, id: EntryId) {
        self.shared.queue.lock().wanted.remove(&id);
    }
}

impl Queue {
    fn next(&mut self) -> Option<Job> {
        loop {
            let Reverse(top) = self.heap.peek()?;
            let current = self.wanted.get(&top.job.id) == Some(&top.job.stamp);
            if current && top.rank.cost == Cost::Import && self.importing {
                return None;
            }
            let Reverse(Queued { job, rank }) = self.heap.pop()?;
            if current {
                self.importing |= rank.cost == Cost::Import;
                return Some(job);
            }
        }
    }
}

fn work(shared: &Shared, deliver: &(dyn Fn(Probe) + Send + Sync)) {
    loop {
        let job = {
            let mut queue = shared.queue.lock();
            loop {
                if queue.closed {
                    return;
                }
                match queue.next() {
                    Some(job) => break job,
                    None => queue = shared.ready.wait(queue),
                }
            }
        };
        let details = probe::read(&job.path, job.format, &shared.cancel);
        if job.format.cost() == Cost::Import {
            shared.queue.lock().importing = false;
            shared.ready.notify_all();
        }
        deliver(Probe {
            id: job.id,
            stamp: job.stamp,
            details,
        });
    }
}
