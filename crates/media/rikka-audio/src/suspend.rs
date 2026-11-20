use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::backend::{Output, Resume};

const SUSPEND_AFTER: Duration = Duration::from_secs(2);

#[derive(Debug, Default)]
pub(crate) struct Activity {
    silent: AtomicBool,
    resume_requested: AtomicBool,
    suspended: AtomicBool,
}

impl Activity {
    pub(crate) fn set_silent(&self, silent: bool) {
        self.silent.store(silent, Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub(crate) struct OutputWaker {
    activity: Arc<Activity>,
    resume: Resume,
}

impl OutputWaker {
    pub(crate) fn new(activity: Arc<Activity>, resume: Resume) -> OutputWaker {
        OutputWaker { activity, resume }
    }

    pub(crate) fn wake(&self) {
        self.activity.resume_requested.store(true, Ordering::SeqCst);
        if self.activity.suspended.load(Ordering::SeqCst) && !self.resume.send() {
            tracing::debug!("audio output is gone");
        }
    }
}

impl std::fmt::Debug for OutputWaker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OutputWaker")
            .field("activity", &self.activity)
            .finish_non_exhaustive()
    }
}

pub(crate) struct Suspender {
    activity: Arc<Activity>,
    silent_since: Option<Instant>,
    can_pause: bool,
}

impl Suspender {
    pub(crate) fn new(activity: Arc<Activity>) -> Suspender {
        Suspender {
            activity,
            silent_since: None,
            can_pause: true,
        }
    }

    pub(crate) fn is_suspended(&self) -> bool {
        self.activity.suspended.load(Ordering::SeqCst)
    }

    pub(crate) fn resume(&mut self, output: &Output) {
        if !self.is_suspended() {
            return;
        }
        if let Err(error) = output.set_active(true) {
            tracing::warn!(%error, "failed to resume suspended audio output");
        }
        self.activity
            .resume_requested
            .store(false, Ordering::SeqCst);
        self.activity.suspended.store(false, Ordering::SeqCst);
        self.silent_since = None;
    }

    pub(crate) fn update(&mut self, output: &Output, now: Instant) {
        if !self.can_pause || self.is_suspended() {
            return;
        }
        let requested = self.activity.resume_requested.swap(false, Ordering::SeqCst);
        if requested || !self.activity.silent.load(Ordering::Relaxed) {
            self.silent_since = None;
            return;
        }
        let since = *self.silent_since.get_or_insert(now);
        if now.saturating_duration_since(since) < SUSPEND_AFTER {
            return;
        }
        if let Err(error) = output.set_active(false) {
            tracing::debug!(%error, "audio output cannot pause, keeping it running");
            self.can_pause = false;
            return;
        }
        self.activity.suspended.store(true, Ordering::SeqCst);
        if self.activity.resume_requested.load(Ordering::SeqCst) {
            self.resume(output);
        }
    }
}
