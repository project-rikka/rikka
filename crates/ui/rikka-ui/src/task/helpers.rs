use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::collections::VecDeque;
use std::sync::{Arc, OnceLock};
use std::thread;

const MAX_HELPERS: usize = 4;

const STACK: usize = 512 * 1024;

type Job = Box<dyn FnOnce() + Send>;

pub(crate) trait Shared: Send + Sync {
    fn work_off(&self);
}

pub fn in_background(job: impl FnOnce() + Send + 'static) {
    helpers().queue(Box::new(job));
}

pub(crate) fn most_hands() -> usize {
    static MOST: OnceLock<usize> = OnceLock::new();
    *MOST.get_or_init(|| {
        thread::available_parallelism()
            .map(|count| count.get().saturating_sub(1).min(MAX_HELPERS))
            .unwrap_or(0)
    })
}

pub(crate) fn helpers() -> &'static Helpers {
    static HELPERS: OnceLock<Helpers> = OnceLock::new();
    HELPERS.get_or_init(|| Helpers {
        pool: Arc::new(Pool {
            waiting: Mutex::new(Waiting::default()),
            ready: Condvar::new(),
        }),
    })
}

#[derive(Debug)]
pub(crate) struct Helpers {
    pool: Arc<Pool>,
}

impl Helpers {
    pub(crate) fn distribute(&self, shared: &Arc<dyn Shared>, hands: usize) {
        let mut waiting = self.pool.waiting();
        waiting
            .shared
            .extend(std::iter::repeat_n(Arc::clone(shared), hands));
        let about = waiting.helpers;
        let short_of = hands.saturating_sub(about);
        waiting.helpers += short_of;
        drop(waiting);
        self.hire(about, short_of);
        self.pool.ready.notify_all();
    }

    pub(crate) fn withdraw(&self, shared: &Arc<dyn Shared>) {
        self.pool
            .waiting()
            .shared
            .retain(|queued| !Arc::ptr_eq(queued, shared));
    }

    fn queue(&self, job: Job) {
        let mut waiting = self.pool.waiting();
        waiting.jobs.push_back(job);
        let about = waiting.helpers;
        let short_of = usize::from(about == 0);
        waiting.helpers += short_of;
        drop(waiting);
        self.hire(about, short_of);
        self.pool.ready.notify_one();
    }

    fn hire(&self, about: usize, short_of: usize) {
        for hand in 0..short_of {
            if !self.pool.clone().hire(about + hand) {
                self.pool.waiting().helpers -= 1;
            }
        }
    }
}

#[derive(Default)]
struct Waiting {
    shared: Vec<Arc<dyn Shared>>,
    jobs: VecDeque<Job>,
    helpers: usize,
}

impl std::fmt::Debug for Waiting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waiting")
            .field("shared", &self.shared.len())
            .field("jobs", &self.jobs.len())
            .field("helpers", &self.helpers)
            .finish()
    }
}

enum Work {
    Shared(Arc<dyn Shared>),
    Job(Job),
}

#[derive(Debug)]
struct Pool {
    waiting: Mutex<Waiting>,
    ready: Condvar,
}

impl Pool {
    fn hire(self: Arc<Self>, hand: usize) -> bool {
        match thread::Builder::new()
            .name(format!("rikka-ui-helper-{hand}"))
            .stack_size(STACK)
            .spawn(move || self.serve())
        {
            Ok(_) => true,
            Err(error) => {
                tracing::warn!("failed to spawn UI helper thread: {error}");
                false
            }
        }
    }

    fn serve(self: Arc<Self>) {
        loop {
            match self.next() {
                Work::Shared(shared) => shared.work_off(),
                Work::Job(job) => job(),
            }
        }
    }

    fn waiting(&self) -> MutexGuard<'_, Waiting> {
        self.waiting.lock()
    }

    fn next(&self) -> Work {
        let mut waiting = self.waiting();
        loop {
            if let Some(shared) = waiting.shared.pop() {
                return Work::Shared(shared);
            }
            if let Some(job) = waiting.jobs.pop_front() {
                return Work::Job(job);
            }
            waiting = self.ready.wait(waiting);
        }
    }
}
