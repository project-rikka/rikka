use std::rc::Rc;
use std::sync::Arc;

use files_core::Location;
use files_undo::{Direction, Kind, Reversal, Step};
use files_vfs::Outcome;
use rikka_file_operations::TaskKind;

use crate::services::SharedServices;
use crate::window::WindowState;
use crate::window::jobs::After;

#[derive(Debug)]
pub(crate) enum Journal {
    Record(TaskKind),
    Reverse(Reversing),
}

pub(crate) struct Reversing {
    shared: Rc<SharedServices>,
    reversal: Option<Reversal>,
}

impl Reversing {
    fn finish(mut self, outcome: &Outcome) {
        if let Some(reversal) = self.reversal.take() {
            self.shared.history.borrow_mut().finish(reversal, outcome);
        }
    }
}

impl Drop for Reversing {
    fn drop(&mut self) {
        if let Some(reversal) = self.reversal.take() {
            self.shared.history.borrow_mut().abandon(reversal);
        }
    }
}

impl std::fmt::Debug for Reversing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reversing")
            .field("reversal", &self.reversal)
            .finish_non_exhaustive()
    }
}

impl WindowState {
    pub(crate) fn undo(&mut self) {
        self.reverse(Direction::Undo);
    }

    pub(crate) fn redo(&mut self) {
        self.reverse(Direction::Redo);
    }

    pub(crate) fn reversible(&self, direction: Direction) -> Option<Kind> {
        if self.jobs.showing().is_some() {
            return None;
        }
        self.services.history.borrow().next(direction)
    }

    fn reverse(&mut self, direction: Direction) {
        if self.jobs.showing().is_some() {
            return;
        }
        let Some(reversal) = self.services.history.borrow_mut().begin(direction) else {
            return;
        };
        let operation = rikka_file_operations::run(Arc::clone(&self.services.vfs), reversal.task());
        let reversing = Reversing {
            shared: self.services.shared(),
            reversal: Some(reversal),
        };
        self.jobs
            .run(operation, After::Journal(Journal::Reverse(reversing)));
    }

    pub(super) fn record(&self, step: Step) {
        self.services.history.borrow_mut().record(step);
    }

    pub(super) fn write_journal(&mut self, journal: Journal, outcome: &Outcome) {
        match journal {
            Journal::Record(kind) => {
                if let Some(step) = Step::from_outcome(kind, outcome) {
                    self.record(step);
                }
            }
            Journal::Reverse(reversing) => {
                reversing.finish(outcome);
                self.select_landed(outcome);
            }
        }
    }

    fn select_landed(&mut self, outcome: &Outcome) {
        if self.tabs.is_empty() {
            return;
        }
        let here = self.tabs.active().at();
        let landed: Vec<Location> = outcome
            .placed
            .iter()
            .filter(|placed| placed.to.parent().as_ref() == Some(here))
            .map(|placed| placed.to.clone())
            .collect();
        if !landed.is_empty() {
            self.tabs.active_mut().select_on_arrival(landed);
        }
    }
}
