mod log;

use installer_sys::{Event, Stage};

pub use log::Log;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageState {
    Done,
    Running,
    Pending,
    Failed,
}

#[derive(Debug, Default)]
pub struct InstallView {
    stages: Vec<Stage>,
    current: Option<Stage>,
    done: Vec<Stage>,
    log: Log,
    outcome: Option<Result<(), String>>,
    frame: usize,
}

impl InstallView {
    pub fn apply(&mut self, event: Event) {
        match event {
            Event::Planned(stages) => self.stages = stages,
            Event::Entered(stage) => {
                if let Some(previous) = self.current.replace(stage) {
                    self.done.push(previous);
                }
            }
            Event::Log(line) => self.log.push(&line),
            Event::Finished => {
                if let Some(stage) = self.current.take() {
                    self.done.push(stage);
                }
                self.outcome = Some(Ok(()));
            }
            Event::Failed(message) => {
                self.log.push(&message);
                self.outcome = Some(Err(message));
            }
        }
    }

    pub fn tick(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    #[must_use]
    pub fn log(&self) -> &Log {
        &self.log
    }

    pub fn log_mut(&mut self) -> &mut Log {
        &mut self.log
    }

    #[must_use]
    pub fn stages(&self) -> &[Stage] {
        &self.stages
    }

    #[must_use]
    pub fn state_of(&self, stage: Stage) -> StageState {
        if self.done.contains(&stage) {
            StageState::Done
        } else if self.current == Some(stage) {
            if self.failed() {
                StageState::Failed
            } else {
                StageState::Running
            }
        } else {
            StageState::Pending
        }
    }

    #[must_use]
    pub fn frame(&self) -> usize {
        self.frame
    }

    #[must_use]
    pub fn outcome(&self) -> Option<&Result<(), String>> {
        self.outcome.as_ref()
    }

    #[must_use]
    pub fn finished(&self) -> bool {
        self.outcome.is_some()
    }

    #[must_use]
    pub fn failed(&self) -> bool {
        matches!(self.outcome, Some(Err(_)))
    }
}
