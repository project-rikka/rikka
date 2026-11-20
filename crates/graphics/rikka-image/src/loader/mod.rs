mod handle;
mod request;
mod task;

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread::JoinHandle;

use rikka_sync::{Condvar, Mutex};
use rikka_types::Source;

pub use handle::LoadHandle;
pub use request::LoadRequest;

use crate::budget::Budget;
use handle::{Decoded, JobState};
use task::Task;

const MAX_DEFAULT_THREADS: usize = 8;

#[derive(Debug)]
pub struct ImageLoader {
    shared: Arc<Shared>,
    workers: Mutex<Vec<JoinHandle<()>>>,
    limit: usize,
    budget: Budget,
    notice: Option<Notice>,
}

#[derive(Clone)]
struct Notice(Arc<dyn Fn() + Send + Sync>);

impl std::fmt::Debug for Notice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Notice").finish_non_exhaustive()
    }
}

impl ImageLoader {
    #[must_use]
    pub fn new() -> ImageLoader {
        let cores = std::thread::available_parallelism().map_or(1, |cores| cores.get());
        ImageLoader::with_threads((cores - 1).clamp(1, MAX_DEFAULT_THREADS))
    }

    #[must_use]
    pub fn with_threads(threads: usize) -> ImageLoader {
        ImageLoader {
            shared: Arc::new(Shared::default()),
            workers: Mutex::new(Vec::new()),
            limit: threads.max(1),
            budget: Budget::default(),
            notice: None,
        }
    }

    #[must_use]
    pub fn with_budget(mut self, budget: Budget) -> ImageLoader {
        self.budget = budget;
        self
    }

    #[must_use]
    pub fn notifying(mut self, notice: impl Fn() + Send + Sync + 'static) -> ImageLoader {
        self.notice = Some(Notice(Arc::new(notice)));
        self
    }

    #[must_use]
    pub fn budget(&self) -> &Budget {
        &self.budget
    }

    fn spawn_worker(&self) {
        let mut workers = self.workers.lock();
        if workers.len() == self.limit {
            return;
        }
        let shared = Arc::clone(&self.shared);
        let worker = std::thread::Builder::new()
            .name(format!("rikka-image-{}", workers.len()))
            .spawn(move || run_worker(&shared))
            .expect("failed to spawn image loader thread");
        workers.push(worker);
    }

    pub fn read<'a>(&self, source: impl Into<Source<'a>>) -> LoadRequest<'_> {
        LoadRequest::new(self, source.into().into_owned())
    }

    pub fn load<'a>(&self, source: impl Into<Source<'a>>) -> LoadHandle {
        self.read(source).submit()
    }

    #[must_use]
    pub fn pending(&self) -> usize {
        self.shared.queue.lock().jobs.len()
    }

    #[must_use]
    pub fn threads(&self) -> usize {
        self.workers.lock().len()
    }
}

impl Default for ImageLoader {
    fn default() -> ImageLoader {
        ImageLoader::new()
    }
}

impl Drop for ImageLoader {
    fn drop(&mut self) {
        self.shared.queue.lock().shutdown = true;
        self.shared.ready.notify_all();
        self.budget.close();
        for worker in self.workers.get_mut().drain(..) {
            if worker.join().is_err() {
                tracing::error!("image loader worker panicked");
            }
        }
    }
}

fn submit(loader: &ImageLoader, task: Task) -> LoadHandle {
    let state = Arc::new(JobState::default());
    let (sender, receiver) = sync_channel(1);

    let mut queue = loader.shared.queue.lock();
    queue.jobs.push_back(Job {
        task,
        delivery: Delivery {
            state: Arc::clone(&state),
            result: sender,
            budget: loader.budget.clone(),
            notice: loader.notice.clone(),
        },
    });
    let waiting = queue.jobs.len() <= queue.idle;
    drop(queue);

    if waiting {
        loader.shared.ready.notify_one();
    } else {
        loader.spawn_worker();
    }

    LoadHandle::new(receiver, state)
}

#[derive(Debug, Default)]
struct Shared {
    queue: Mutex<Queue>,
    ready: Condvar,
}

#[derive(Debug, Default)]
struct Queue {
    jobs: VecDeque<Job>,
    shutdown: bool,
    idle: usize,
}

#[derive(Debug)]
struct Job {
    task: Task,
    delivery: Delivery,
}

#[derive(Debug)]
struct Delivery {
    state: Arc<JobState>,
    result: SyncSender<Decoded>,
    budget: Budget,
    notice: Option<Notice>,
}

impl Drop for Delivery {
    fn drop(&mut self) {
        self.state.finish();
        if let Some(Notice(notice)) = &self.notice {
            notice();
        }
    }
}

fn run_worker(shared: &Shared) {
    loop {
        let job = {
            let mut queue = shared.queue.lock();
            loop {
                if let Some(job) = queue.jobs.pop_front() {
                    break job;
                }
                if queue.shutdown {
                    return;
                }
                queue.idle += 1;
                queue = shared.ready.wait(queue);
                queue.idle -= 1;
            }
        };

        let Job { task, delivery } = job;
        if delivery.state.is_cancelled() {
            continue;
        }
        if let Some(decoded) = task.run(&delivery.budget, &delivery.state) {
            delivery.result.send(decoded).ok();
        }
    }
}
