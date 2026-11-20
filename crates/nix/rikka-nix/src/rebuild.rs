use rikka_nix_protocol::Request;

use crate::apply::run;
use crate::error::{Error, Result};
use crate::outcome::{Outcome, Progress, Refusal};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Idle,
    Requested,
    Running,
}

impl Stage {
    #[must_use]
    pub fn is_running(self) -> bool {
        self != Self::Idle
    }
}

#[derive(Debug, Default)]
pub struct Rebuild {
    stage: Stage,
    refusal: Option<Refusal>,
}

impl Rebuild {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn stage(&self) -> Stage {
        self.stage
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        self.stage.is_running()
    }

    #[must_use]
    pub fn refusal(&self) -> Option<&Refusal> {
        self.refusal.as_ref()
    }

    pub fn dismiss_refusal(&mut self) {
        self.refusal = None;
    }

    pub fn start(&mut self, request: Request) -> PendingRequest {
        self.stage = Stage::Requested;
        self.refusal = None;
        PendingRequest(request)
    }

    pub fn fail(&mut self, error: &Error) {
        self.conclude(&Outcome::Refused(Refusal::from(error)));
    }

    pub fn handle_progress(&mut self, progress: Progress) -> Option<Outcome> {
        if !self.is_running() {
            return None;
        }
        match progress {
            Progress::Started => {
                self.stage = Stage::Running;
                None
            }
            Progress::Finished(outcome) => {
                self.conclude(&outcome);
                Some(outcome)
            }
        }
    }

    fn conclude(&mut self, outcome: &Outcome) {
        self.stage = Stage::Idle;
        self.refusal = outcome.refusal().cloned();
    }
}

#[must_use]
#[derive(Debug)]
pub struct PendingRequest(Request);

impl PendingRequest {
    pub fn send(self, report: impl Fn(Progress) + Send + 'static) -> Result<()> {
        run(&self.0, report)
    }
}
