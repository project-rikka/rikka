use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use crate::backend::{self, Output, OutputHost};
use crate::config::EngineConfig;
use crate::error::Result;
use crate::lost::DeviceLost;
use crate::mixer::{Mixer, Retired};
use crate::output::OutputPlan;
use crate::suspend::{Activity, OutputWaker, Suspender};
use crate::watch::Watched;

pub(crate) const THREAD: &str = "rikka-audio";
const RETIRED_SWEEP_INTERVAL: Duration = Duration::from_millis(50);
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub(crate) struct StreamHost {
    waker: OutputWaker,
    watched: Arc<Watched>,
    _host: OutputHost,
}

pub(crate) struct Shared {
    pub(crate) activity: Arc<Activity>,
    pub(crate) device_lost: Arc<DeviceLost>,
    pub(crate) latency: Arc<AtomicU64>,
    pub(crate) watched: Arc<Watched>,
}

impl StreamHost {
    pub(crate) fn start(
        config: EngineConfig,
        mix: impl FnOnce(&OutputPlan) -> Mixer + Send + 'static,
        shared: Shared,
        retired: rikka_ring::Consumer<Retired>,
    ) -> Result<(StreamHost, OutputPlan)> {
        let (resume, resumes) = backend::resume_channel()?;
        let waker = OutputWaker::new(Arc::clone(&shared.activity), resume);
        let watched = Arc::clone(&shared.watched);
        let (host, plan) = OutputHost::start(config, mix, shared, retired, resumes)?;
        Ok((
            StreamHost {
                waker,
                watched,
                _host: host,
            },
            plan,
        ))
    }

    pub(crate) fn waker(&self) -> &OutputWaker {
        &self.waker
    }

    pub(crate) fn watched(&self) -> Weak<Watched> {
        Arc::downgrade(&self.watched)
    }
}

pub(crate) struct Chores {
    suspender: Suspender,
    retired: rikka_ring::Consumer<Retired>,
    watched: Arc<Watched>,
    reported: Instant,
}

impl Chores {
    pub(crate) fn new(
        activity: Arc<Activity>,
        retired: rikka_ring::Consumer<Retired>,
        watched: Arc<Watched>,
    ) -> Chores {
        Chores {
            suspender: Suspender::new(activity),
            retired,
            watched,
            reported: Instant::now(),
        }
    }

    pub(crate) fn next_wake(&self) -> Option<Duration> {
        match self.suspender.is_suspended() {
            true => None,
            false => Some(RETIRED_SWEEP_INTERVAL),
        }
    }

    #[cfg(windows)]
    pub(crate) fn is_suspended(&self) -> bool {
        self.suspender.is_suspended()
    }

    pub(crate) fn resume(&mut self, output: &Output) {
        self.suspender.resume(output);
    }

    pub(crate) fn tend(&mut self, output: &Output) {
        while let Some(voice) = self.retired.pop() {
            drop(voice);
        }
        if self.reported.elapsed() >= PROGRESS_EVERY {
            self.reported = Instant::now();
            self.watched.report();
        }
        self.suspender.update(output, Instant::now());
    }
}
