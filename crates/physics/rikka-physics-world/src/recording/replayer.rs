use std::sync::Arc;

use rikka_physics_core::SerialExecutor;

use super::{Entry, Recording};
use crate::call::Call;
use crate::{Result, World};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub tick: u64,
    pub expected: Option<u64>,
    pub actual: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplayReport {
    pub ticks: u64,
    pub divergence: Option<Divergence>,
}

#[derive(Debug)]
pub struct Replayer<'r> {
    recording: &'r Recording,
    world: World,
    cursor: usize,
    ticks: u64,
    divergence: Option<Divergence>,
}

impl<'r> Replayer<'r> {
    pub(super) fn new(recording: &'r Recording) -> Result<Self> {
        let world = World::new(*recording.settings(), Arc::new(SerialExecutor))?;
        Ok(Self {
            recording,
            world,
            cursor: 0,
            ticks: 0,
            divergence: None,
        })
    }

    pub fn world(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn next_frame(&mut self) -> Result<bool> {
        if self.divergence.is_some() {
            return Ok(false);
        }
        let entries = self.recording.entries();
        while let Some(entry) = entries.get(self.cursor) {
            self.cursor += 1;
            match entry {
                Entry::Setup(setup) => self.world.apply_setup(setup)?,
                Entry::Call(call @ (Call::Advance(_) | Call::AdvanceTo(_))) => {
                    self.world.replay_call(call)?;
                    self.compare();
                    return Ok(self.divergence.is_none());
                }
                Entry::Call(call) => self.world.replay_call(call)?,
                Entry::Tick(expected) => {
                    self.ticks += 1;
                    self.divergence = Some(Divergence {
                        tick: self.ticks,
                        expected: Some(*expected),
                        actual: None,
                    });
                    return Ok(false);
                }
            }
        }
        Ok(false)
    }

    pub fn report(&self) -> ReplayReport {
        ReplayReport {
            ticks: self.ticks,
            divergence: self.divergence,
        }
    }

    pub fn run(mut self) -> Result<ReplayReport> {
        while self.next_frame()? {}
        Ok(self.report())
    }

    fn compare(&mut self) {
        let entries = self.recording.entries();
        let mut produced = self.world.tick_hashes().iter().copied();
        loop {
            let expected = match entries.get(self.cursor) {
                Some(Entry::Tick(hash)) => Some(*hash),
                _ => None,
            };
            let actual = produced.next();
            if expected.is_none() && actual.is_none() {
                return;
            }
            if expected.is_some() {
                self.cursor += 1;
            }
            self.ticks += 1;
            if expected != actual {
                self.divergence = Some(Divergence {
                    tick: self.ticks,
                    expected,
                    actual,
                });
                return;
            }
        }
    }
}
