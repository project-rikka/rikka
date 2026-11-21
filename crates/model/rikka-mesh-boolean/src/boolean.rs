use std::num::NonZeroUsize;

use rikka_parallel::Threads;
use rikka_polymesh::Polymesh;

use crate::error::{Error, Result};
use crate::exact_boolean::{self, ExactOptions};
use crate::fast;
use crate::operand::Operand;
use crate::operation::{Operation, Solver};

pub const DEFAULT_THRESHOLD: f32 = 1.0e-6;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FaceOrigin {
    pub operand: u32,
    pub face: u32,
}

#[derive(Clone, Debug)]
pub struct BooleanResult {
    pub mesh: Polymesh,
    pub face_origins: Vec<FaceOrigin>,
    pub intersecting_edges: Vec<u32>,
}

#[must_use]
#[derive(Clone, Debug)]
pub struct Boolean<'a> {
    operation: Operation,
    operands: Vec<Operand<'a>>,
    solver: Solver,
    self_intersection: bool,
    hole_tolerant: bool,
    threshold: f32,
    threads: Option<NonZeroUsize>,
}

impl<'a> Boolean<'a> {
    pub fn new(operation: Operation) -> Boolean<'a> {
        Boolean {
            operation,
            operands: Vec::new(),
            solver: Solver::Exact,
            self_intersection: false,
            hole_tolerant: false,
            threshold: DEFAULT_THRESHOLD,
            threads: None,
        }
    }

    pub fn operand(mut self, operand: impl Into<Operand<'a>>) -> Boolean<'a> {
        self.operands.push(operand.into());
        self
    }

    pub fn operands<O: Into<Operand<'a>>>(
        mut self,
        operands: impl IntoIterator<Item = O>,
    ) -> Boolean<'a> {
        self.operands.extend(operands.into_iter().map(Into::into));
        self
    }

    pub fn solver(mut self, solver: Solver) -> Boolean<'a> {
        self.solver = solver;
        self
    }

    pub fn self_intersection(mut self, enabled: bool) -> Boolean<'a> {
        self.self_intersection = enabled;
        self
    }

    pub fn hole_tolerant(mut self, enabled: bool) -> Boolean<'a> {
        self.hole_tolerant = enabled;
        self
    }

    pub fn threshold(mut self, threshold: f32) -> Boolean<'a> {
        self.threshold = threshold;
        self
    }

    pub fn threads(mut self, threads: NonZeroUsize) -> Boolean<'a> {
        self.threads = Some(threads);
        self
    }

    fn validate(&self) -> Result<()> {
        if self.operands.is_empty() {
            return Err(Error::NoOperands);
        }
        for (index, operand) in self.operands.iter().enumerate() {
            operand
                .mesh
                .validate()
                .map_err(|error| Error::InvalidOperand {
                    operand: index,
                    error,
                })?;
            if !operand.transform.is_finite() {
                return Err(Error::NonFiniteTransform { operand: index });
            }
        }
        if !(self.threshold >= 0.0 && self.threshold.is_finite()) {
            return Err(Error::InvalidThreshold(self.threshold));
        }
        Ok(())
    }

    pub fn compute(&self) -> Result<BooleanResult> {
        self.validate()?;
        let threads = self.threads.map_or_else(Threads::available, Threads::from);
        match self.solver {
            Solver::Exact => Ok(exact_boolean::boolean(
                &self.operands,
                &ExactOptions {
                    operation: self.operation,
                    use_self: self.self_intersection,
                    hole_tolerant: self.hole_tolerant,
                    threads,
                },
            )),
            Solver::Fast => fast::boolean(&self.operands, self.operation, self.threshold),
        }
    }
}

impl<'a> From<&'a Polymesh> for Operand<'a> {
    fn from(mesh: &'a Polymesh) -> Operand<'a> {
        Operand::new(mesh)
    }
}
