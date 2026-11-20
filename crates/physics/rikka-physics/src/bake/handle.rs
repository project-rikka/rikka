use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::sync::Arc;
use std::thread::JoinHandle;

use rikka_gpu::Compute;

use super::cache::{Cache, DEFAULT_BUDGET, DEFAULT_CHECKPOINT_INTERVAL};
use super::replay::SceneReplay;
use super::state::{BakeState, BakeStatus, Progress};
use super::worker;
use crate::error::{Error, Result};
use crate::scene_source::SceneSource;

const THREAD: &str = "rikka-physics-bake";

pub type ProgressCallback = Arc<dyn Fn() + Send + Sync>;

pub(super) struct Shared {
    state: Mutex<BakeState>,
    changed: Condvar,
    cache: Mutex<Cache>,
    pub(super) notify: ProgressCallback,
}

impl Shared {
    pub(super) fn state(&self) -> MutexGuard<'_, BakeState> {
        self.state.lock()
    }

    pub(super) fn cache(&self) -> MutexGuard<'_, Cache> {
        self.cache.lock()
    }

    pub(super) fn wait<'a>(&self, state: MutexGuard<'a, BakeState>) -> MutexGuard<'a, BakeState> {
        self.changed.wait(state)
    }

    fn update(&self, change: impl FnOnce(&mut BakeState)) {
        change(&mut self.state());
        self.changed.notify_all();
    }
}

pub(super) enum ShowStatus {
    Shown,
    Missing,
    Busy,
}

#[must_use]
#[derive(Clone, Debug)]
pub struct BakeBuilder {
    thread: String,
    budget: usize,
    checkpoint_every: u32,
}

impl BakeBuilder {
    pub fn thread(mut self, name: impl Into<String>) -> BakeBuilder {
        self.thread = name.into();
        self
    }

    pub fn budget(mut self, bytes: usize) -> BakeBuilder {
        self.budget = bytes;
        self
    }

    pub fn checkpoint_every(mut self, steps: u32) -> BakeBuilder {
        self.checkpoint_every = steps.max(1);
        self
    }

    pub fn spawn(self, on_progress: ProgressCallback) -> Result<Bake> {
        let shared = Arc::new(Shared {
            state: Mutex::new(BakeState::default()),
            changed: Condvar::new(),
            cache: Mutex::new(Cache::new(self.budget, self.checkpoint_every)),
            notify: on_progress,
        });
        let working = Arc::clone(&shared);
        let worker = std::thread::Builder::new()
            .name(self.thread)
            .spawn(move || worker::run(&working))
            .map_err(Error::Thread)?;
        Ok(Bake {
            shared,
            worker: Some(worker),
        })
    }
}

pub struct Bake {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl Bake {
    pub fn builder() -> BakeBuilder {
        BakeBuilder {
            thread: THREAD.to_owned(),
            budget: DEFAULT_BUDGET,
            checkpoint_every: DEFAULT_CHECKPOINT_INTERVAL,
        }
    }

    pub fn set_compute(&self, compute: &Compute) {
        if self.shared.state().compute.is_none() {
            self.shared
                .update(|state| state.compute = Some(compute.background()));
        }
    }

    pub fn set_source(&self, source: Arc<dyn SceneSource>, since: Option<u32>) {
        self.shared.update(|state| state.set_source(source, since));
    }

    pub fn pause(&self, paused: bool) {
        self.shared.update(|state| state.paused = paused);
        (self.shared.notify)();
    }

    pub fn recompute(&self) {
        self.shared.update(|state| {
            state.halted = false;
            if let Some(source) = state.source.clone() {
                state.set_source(source, Some(0));
            }
        });
    }

    pub fn set_target_step(&self, step: u32) {
        let mut state = self.shared.state();
        if state.target_step != step {
            state.target_step = step;
            drop(state);
            self.shared.changed.notify_all();
        }
    }

    #[must_use]
    pub fn progress(&self) -> Progress {
        self.shared.state().progress()
    }

    pub fn take_error(&self) -> Option<Error> {
        self.shared.state().error.take()
    }

    pub(super) fn status(&self, step: u32) -> BakeStatus {
        self.shared.state().status(step)
    }

    pub(super) fn watch(&self, step: u32) -> BakeStatus {
        let mut state = self.shared.state();
        let moved = state.set_playhead(step);
        let status = state.status(step);
        drop(state);
        if moved {
            self.shared.changed.notify_all();
        }
        status
    }

    pub(super) fn show(&self, step: u32, replay: &mut SceneReplay) -> Result<ShowStatus> {
        let capture = match self.shared.cache.try_lock() {
            Some(cache) => cache.capture(step),
            None => return Ok(ShowStatus::Busy),
        };
        let Some(capture) = capture else {
            return Ok(ShowStatus::Missing);
        };
        if !capture.is_ready() {
            return Ok(ShowStatus::Busy);
        }
        replay.show(&capture)?;
        Ok(ShowStatus::Shown)
    }
}

impl Drop for Bake {
    fn drop(&mut self) {
        self.shared.update(|state| state.quit = true);
        if let Some(worker) = self.worker.take()
            && let Err(payload) = worker.join()
            && !std::thread::panicking()
        {
            std::panic::resume_unwind(payload);
        }
    }
}

impl std::fmt::Debug for Bake {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bake").finish_non_exhaustive()
    }
}
