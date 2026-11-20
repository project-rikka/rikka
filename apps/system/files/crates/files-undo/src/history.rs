use std::collections::HashSet;

use files_core::Location;
use files_vfs::{Outcome, Task};

use crate::change::Change;
use crate::kind::Kind;
use crate::step::Step;

const DEPTH: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Undo,
    Redo,
}

impl Direction {
    fn opposite(self) -> Self {
        match self {
            Self::Undo => Self::Redo,
            Self::Redo => Self::Undo,
        }
    }
}

#[derive(Debug)]
#[must_use]
pub struct Reversal {
    direction: Direction,
    step: Step,
    records: u64,
}

impl Reversal {
    pub fn task(&self) -> Task {
        self.step.change.reversal()
    }
}

#[derive(Debug, Default)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    reversing: bool,
    records: u64,
}

impl History {
    pub fn record(&mut self, step: Step) {
        self.records += 1;
        self.redo.clear();
        self.undo.push(step);
        trim(&mut self.undo);
    }

    #[must_use]
    pub fn next(&self, direction: Direction) -> Option<Kind> {
        if self.reversing {
            return None;
        }
        self.stack(direction).last().map(|step| step.kind)
    }

    pub fn begin(&mut self, direction: Direction) -> Option<Reversal> {
        if self.reversing {
            return None;
        }
        let step = self.stack_mut(direction).pop()?;
        self.reversing = true;
        Some(Reversal {
            direction,
            step,
            records: self.records,
        })
    }

    pub fn finish(&mut self, reversal: Reversal, outcome: &Outcome) {
        self.reversing = false;
        let Reversal {
            direction,
            step,
            records,
        } = reversal;
        let kind = step.kind;
        if let Some(change) = Change::from_outcome(step.change.reversal().kind(), outcome) {
            self.put(direction.opposite(), Step { kind, change }, records);
        }
        if !outcome.cancelled {
            return;
        }
        let touched: HashSet<Location> = outcome
            .placed
            .iter()
            .map(|placed| &placed.from)
            .chain(outcome.failures.iter().map(|failure| &failure.at))
            .flat_map(|at| std::iter::successors(Some(at.clone()), Location::parent))
            .collect();
        let mut untouched = step.change;
        untouched.retain(|landed| !touched.contains(landed));
        if !untouched.is_empty() {
            self.put(
                direction,
                Step {
                    kind,
                    change: untouched,
                },
                records,
            );
        }
    }

    pub fn abandon(&mut self, reversal: Reversal) {
        self.reversing = false;
        self.put(reversal.direction, reversal.step, reversal.records);
    }

    fn put(&mut self, direction: Direction, step: Step, records: u64) {
        let since = usize::try_from(self.records - records).unwrap_or(usize::MAX);
        match direction {
            Direction::Undo => {
                let at = self.undo.len().saturating_sub(since);
                self.undo.insert(at, step);
                trim(&mut self.undo);
            }
            Direction::Redo if since > 0 => {}
            Direction::Redo => {
                self.redo.push(step);
                trim(&mut self.redo);
            }
        }
    }

    fn stack(&self, direction: Direction) -> &Vec<Step> {
        match direction {
            Direction::Undo => &self.undo,
            Direction::Redo => &self.redo,
        }
    }

    fn stack_mut(&mut self, direction: Direction) -> &mut Vec<Step> {
        match direction {
            Direction::Undo => &mut self.undo,
            Direction::Redo => &mut self.redo,
        }
    }
}

fn trim(steps: &mut Vec<Step>) {
    if let Some(excess) = steps.len().checked_sub(DEPTH) {
        steps.drain(..excess);
    }
}
