use rikka_nix::{PendingRequest, Progress, Rebuild, Refusal, Request, Stage};

use crate::job::Job;

#[derive(Debug)]
pub(crate) struct Work {
    job: Option<Job>,
    preparing: bool,
    rebuild: Rebuild,
}

impl Work {
    pub(crate) fn new() -> Self {
        Self {
            job: None,
            preparing: false,
            rebuild: Rebuild::new(),
        }
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.preparing || self.rebuild.is_running()
    }

    pub(crate) fn running(&self, job: Job) -> Option<Stage> {
        if !self.is_busy() || self.job != Some(job) {
            return None;
        }
        Some(match self.preparing {
            true => Stage::Requested,
            false => self.rebuild.stage(),
        })
    }

    pub(crate) fn refusal(&self) -> Option<&Refusal> {
        self.rebuild.refusal()
    }

    pub(crate) fn start(&mut self, job: Job) {
        self.job = Some(job);
        self.preparing = true;
        self.rebuild.dismiss_refusal();
    }

    pub(crate) fn begin(&mut self, request: Request) -> PendingRequest {
        self.preparing = false;
        self.rebuild.start(request)
    }

    pub(crate) fn failed(&mut self, error: &rikka_nix::Error) {
        self.preparing = false;
        self.rebuild.fail(error);
    }

    pub(crate) fn progress(&mut self, progress: Progress) -> bool {
        self.rebuild.handle_progress(progress).is_some()
    }

    pub(crate) fn dismiss(&mut self) {
        self.rebuild.dismiss_refusal();
    }
}
