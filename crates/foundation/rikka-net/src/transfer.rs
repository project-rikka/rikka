use rikka_sync::Mutex;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tokio::runtime::Handle;
use tokio::task::JoinHandle;

use crate::Flow;
use crate::cancel::CancelToken;
use crate::error::Result;
use crate::progress::{Progress, ProgressCallback};
use crate::runtime;

pub struct Transfer<T> {
    state: Arc<State<T>>,
    task: JoinHandle<()>,
}

struct State<T> {
    progress: Arc<Mutex<Progress>>,
    started: Instant,
    elapsed: Mutex<Option<Duration>>,
    finished: AtomicBool,
    outcome: Mutex<Option<Result<T>>>,
    watcher: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    cancel: CancelToken,
}

impl<T> State<T> {
    fn finish(&self, outcome: Result<T>) {
        *self.elapsed.lock() = Some(self.started.elapsed());
        *self.outcome.lock() = Some(outcome);
        self.finished.store(true, Ordering::Release);

        let callback = self.watcher.lock().take();
        if let Some(callback) = callback {
            callback();
        }
    }
}

impl<T: Send + 'static> Transfer<T> {
    #[must_use]
    pub fn start<F>(cancel: CancelToken, build: impl FnOnce(ProgressSink) -> F) -> Transfer<T>
    where
        F: Future<Output = Result<T>> + Send + 'static,
    {
        let progress = Arc::new(Mutex::new(Progress::default()));
        let started = Instant::now();
        let work = build(ProgressSink {
            progress: Arc::clone(&progress),
            started,
        });

        Transfer::spawn_on(runtime::shared().handle().clone(), progress, cancel, work)
    }

    pub(crate) fn spawn<F>(
        progress: Arc<Mutex<Progress>>,
        cancel: CancelToken,
        work: F,
    ) -> Transfer<T>
    where
        F: Future<Output = Result<T>> + Send + 'static,
    {
        Transfer::spawn_on(runtime::handle(), progress, cancel, work)
    }

    fn spawn_on<F>(
        runtime: Handle,
        progress: Arc<Mutex<Progress>>,
        cancel: CancelToken,
        work: F,
    ) -> Transfer<T>
    where
        F: Future<Output = Result<T>> + Send + 'static,
    {
        let state = Arc::new(State {
            progress,
            started: Instant::now(),
            elapsed: Mutex::new(None),
            finished: AtomicBool::new(false),
            outcome: Mutex::new(None),
            watcher: Mutex::new(None),
            cancel,
        });

        let worker = Arc::clone(&state);
        let task = runtime.spawn(async move {
            let outcome = work.await;
            worker.finish(outcome);
        });

        Transfer { state, task }
    }

    pub async fn wait(self) -> Result<T> {
        let Transfer { state, task } = self;
        if let Err(error) = task.await {
            std::panic::resume_unwind(error.into_panic());
        }
        state
            .outcome
            .lock()
            .take()
            .expect("finished transfer has a result")
    }

    pub fn join(self) -> Result<T> {
        runtime::block_on(self.wait())
    }
}

impl<T> Transfer<T> {
    #[must_use]
    pub fn progress(&self) -> Progress {
        *self.state.progress.lock()
    }

    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.state
            .elapsed
            .lock()
            .unwrap_or_else(|| self.state.started.elapsed())
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.state.finished.load(Ordering::Acquire)
    }

    pub fn cancel(&self) {
        self.state.cancel.cancel();
    }

    #[must_use]
    pub fn cancel_token(&self) -> CancelToken {
        self.state.cancel.clone()
    }

    pub fn on_finish(&self, callback: impl FnOnce() + Send + 'static) {
        let mut watcher = self.state.watcher.lock();
        if self.is_finished() {
            drop(watcher);
            callback();
            return;
        }
        *watcher = Some(Box::new(callback));
    }

    #[must_use]
    pub fn take_result_if(&self, predicate: impl FnOnce(&Result<T>) -> bool) -> Option<Result<T>> {
        let mut slot = self.state.outcome.lock();
        match slot.as_ref() {
            Some(outcome) if predicate(outcome) => slot.take(),
            _ => None,
        }
    }
}

impl<T> fmt::Debug for Transfer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Transfer")
            .field("finished", &self.is_finished())
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone)]
pub struct ProgressSink {
    progress: Arc<Mutex<Progress>>,
    started: Instant,
}

impl ProgressSink {
    pub fn callback(&self) -> ProgressCallback {
        share(&self.progress, None)
    }

    pub fn finish(&self, transferred: u64, total: Option<u64>) {
        *self.progress.lock() = Progress::finished(transferred, total, self.started.elapsed());
    }
}

pub(crate) fn share(
    shared: &Arc<Mutex<Progress>>,
    callback: Option<ProgressCallback>,
) -> ProgressCallback {
    let shared = Arc::clone(shared);
    let mut callback = callback;

    Box::new(move |progress| {
        *shared.lock() = progress;

        match callback.as_mut() {
            Some(callback) => callback(progress),
            None => Flow::Continue,
        }
    })
}
