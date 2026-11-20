use std::fmt;

use rikka_physics_core::Workers;
use rikka_physics_geometry::Tolerance;
use rikka_physics_math::{Simd, Vec3};

use crate::settings::Settings;
use crate::{Error, Result, World};

const DEFAULT_FRAME_BUDGET: f64 = 0.1;

#[must_use]
pub struct WorldBuilder {
    gravity: Vec3,
    tick_rate: u32,
    substeps: u32,
    frame_budget: f64,
    tolerance: Tolerance,
    simd: Option<Simd>,
    workers: Workers,
}

impl fmt::Debug for WorldBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorldBuilder")
            .field("gravity", &self.gravity)
            .field("tick_rate", &self.tick_rate)
            .field("substeps", &self.substeps)
            .field("frame_budget", &self.frame_budget)
            .field("tolerance", &self.tolerance)
            .field("simd", &self.simd)
            .field("workers", &self.workers)
            .finish()
    }
}

impl WorldBuilder {
    pub(crate) fn new() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            tick_rate: 120,
            substeps: 2,
            frame_budget: DEFAULT_FRAME_BUDGET,
            tolerance: Tolerance::default(),
            simd: None,
            workers: Workers::default(),
        }
    }

    pub fn gravity(mut self, gravity: Vec3) -> Self {
        self.gravity = gravity;
        self
    }

    pub fn tick_rate(mut self, hertz: u32) -> Self {
        self.tick_rate = hertz;
        self
    }

    pub fn substeps(mut self, substeps: u32) -> Self {
        self.substeps = substeps;
        self
    }

    pub fn game(self) -> Self {
        self.tick_rate(60).substeps(4)
    }

    pub fn frame_budget(mut self, seconds: f64) -> Self {
        self.frame_budget = seconds;
        self
    }

    pub fn tolerance(mut self, tolerance: Tolerance) -> Self {
        self.tolerance = tolerance;
        self
    }

    pub fn simd(mut self, simd: Simd) -> Self {
        self.simd = Some(simd);
        self
    }

    pub fn workers(mut self, workers: impl Into<Workers>) -> Self {
        self.workers = workers.into();
        self
    }

    pub fn build(self) -> Result<World> {
        if self.tick_rate == 0 {
            return Err(Error::NoTickRate);
        }
        if self.substeps == 0 {
            return Err(Error::NoSubsteps);
        }
        if !self.gravity.is_finite() {
            return Err(Error::NotFinite { what: "gravity" });
        }
        if !(self.frame_budget.is_finite() && self.frame_budget > 0.0) {
            return Err(Error::NotPositive {
                what: "frame budget",
                value: self.frame_budget,
            });
        }
        let length_scale = self.tolerance.length_scale;
        if !(length_scale.is_finite() && length_scale > 0.0) {
            return Err(Error::NotPositive {
                what: "length scale",
                value: length_scale as f64,
            });
        }
        let simd = self.simd.unwrap_or_else(Simd::detect);
        let executor = self.workers.build()?;
        let max_ticks = ((self.frame_budget * self.tick_rate as f64).round() as u32).max(1);
        let settings = Settings {
            gravity: self.gravity,
            tick_rate: self.tick_rate,
            substeps: self.substeps,
            max_ticks,
            tolerance: self.tolerance,
            simd,
        };
        World::new(settings, executor)
    }
}
