use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use discord_krisp::{Module, Sdk};
use rikka_sync::Mutex;
use rikka_worker::{Broadcast, Subscription};

use crate::error::{Error, Result};

const THREAD: &str = "rikka-discord-krisp";
const WORKING: &str = "work";
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, PartialEq)]
pub enum KrispState {
    Idle,
    Preparing,
    Downloading { received: u64, total: Option<u64> },
    Ready,
    Unavailable(String),
}

impl KrispState {
    #[must_use]
    pub fn percent(&self) -> Option<u8> {
        match self {
            KrispState::Downloading {
                received,
                total: Some(total),
            } if *total > 0 => Some((received.saturating_mul(100) / total).min(100) as u8),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct Krisp {
    shared: Arc<Shared>,
}

struct Shared {
    cache: PathBuf,
    status: Mutex<Status>,
    sdk: OnceLock<Sdk>,
}

struct Status {
    state: KrispState,
    watchers: Broadcast<KrispState>,
}

impl Krisp {
    pub fn new(cache: impl Into<PathBuf>) -> Krisp {
        Krisp {
            shared: Arc::new(Shared {
                cache: cache.into(),
                status: Mutex::new(Status {
                    state: KrispState::Idle,
                    watchers: Broadcast::new(),
                }),
                sdk: OnceLock::new(),
            }),
        }
    }

    pub fn prepare(&self) -> Result<()> {
        {
            let mut status = self.shared.status.lock();
            if !matches!(status.state, KrispState::Idle | KrispState::Unavailable(_)) {
                return Ok(());
            }
            status.change(KrispState::Preparing);
        }
        let shared = Arc::clone(&self.shared);
        let started = std::thread::Builder::new()
            .name(THREAD.into())
            .spawn(move || shared.load());
        match started {
            Ok(_) => Ok(()),
            Err(source) => {
                self.shared.publish(KrispState::Idle);
                Err(Error::Thread {
                    name: THREAD,
                    source,
                })
            }
        }
    }

    #[must_use]
    pub fn state(&self) -> KrispState {
        self.shared.status.lock().state.clone()
    }

    pub fn watch(&self) -> Subscription<KrispState> {
        let (subscriber, subscription) = rikka_worker::subscription();
        let status = self.shared.status.lock();
        subscriber.send(status.state.clone());
        status.watchers.add(subscriber);
        subscription
    }

    pub(crate) fn sdk(&self) -> Option<&Sdk> {
        self.shared.sdk.get()
    }
}

impl Status {
    fn change(&mut self, state: KrispState) {
        self.watchers.send(&state);
        self.state = state;
    }
}

impl Shared {
    fn load(self: Arc<Shared>) {
        if let Some(cached) = Module::cached(&self.cache) {
            cached.prune_others(&self.cache);
            self.start(&cached);
            if let Err(error) = Module::fetch(&self.cache, |_, _| {}) {
                tracing::debug!("failed to refresh Krisp: {error}");
            }
            return;
        }
        let shared = Arc::clone(&self);
        let mut last_progress: Option<Instant> = None;
        let fetched = Module::fetch(&self.cache, move |received, total| {
            if last_progress.is_none_or(|at| at.elapsed() >= PROGRESS_EVERY) {
                last_progress = Some(Instant::now());
                shared.publish(KrispState::Downloading { received, total });
            }
        });
        match fetched {
            Ok(module) => self.start(&module),
            Err(error) => self.fail(&error),
        }
    }

    fn start(&self, module: &Module) {
        tracing::info!(
            folder = %module.folder().display(),
            model = %module.noise_model().display(),
            "Krisp loaded"
        );
        match Sdk::load(module, &self.cache.join(WORKING)) {
            Ok(sdk) => {
                let _ = self.sdk.set(sdk);
                self.publish(KrispState::Ready);
            }
            Err(error) => self.fail(&error),
        }
    }

    fn fail(&self, error: &discord_krisp::Error) {
        tracing::warn!("Krisp unavailable: {error}");
        self.publish(KrispState::Unavailable(error.to_string()));
    }

    fn publish(&self, state: KrispState) {
        self.status.lock().change(state);
    }
}

impl std::fmt::Debug for Krisp {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Krisp")
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}
