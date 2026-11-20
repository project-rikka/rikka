use rikka_physics_core::Workers;
use rikka_physics_geometry::Tolerance;
use rikka_physics_math::{Simd, Vec3};

use crate::config::Config;
use crate::{Error, Result, RigidWorld, Spring};

#[must_use]
pub struct RigidWorldBuilder {
    config: Config,
    workers: Workers,
    simd: Option<Simd>,
}

impl std::fmt::Debug for RigidWorldBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RigidWorldBuilder")
            .field("config", &self.config)
            .field("workers", &self.workers)
            .field("simd", &self.simd)
            .finish()
    }
}

impl RigidWorldBuilder {
    pub(crate) fn new() -> Self {
        Self {
            config: Config::default(),
            workers: Workers::default(),
            simd: None,
        }
    }

    pub fn gravity(mut self, gravity: Vec3) -> Self {
        self.config.gravity = gravity;
        self
    }

    pub fn substeps(mut self, substeps: u32) -> Self {
        self.config.substeps = substeps;
        self
    }

    pub fn workers(mut self, workers: impl Into<Workers>) -> Self {
        self.workers = workers.into();
        self
    }

    pub fn simd(mut self, simd: Simd) -> Self {
        self.simd = Some(simd);
        self
    }

    pub fn tolerance(mut self, tolerance: Tolerance) -> Self {
        self.config.tolerance = tolerance;
        self
    }

    pub fn contact_softness(mut self, spring: Spring) -> Self {
        self.config.contact_softness = spring;
        self
    }

    pub fn contact_push_speed(mut self, speed: f32) -> Self {
        self.config.contact_push_speed = speed;
        self
    }

    pub fn joint_softness(mut self, spring: Spring) -> Self {
        self.config.joint_softness = spring;
        self
    }

    pub fn restitution_threshold(mut self, speed: f32) -> Self {
        self.config.restitution_threshold = speed;
        self
    }

    pub fn hit_event_threshold(mut self, speed: f32) -> Self {
        self.config.hit_event_threshold = speed;
        self
    }

    pub fn max_linear_speed(mut self, speed: f32) -> Self {
        self.config.max_linear_speed = speed;
        self
    }

    pub fn sleeping(mut self, enabled: bool) -> Self {
        self.config.sleeping = enabled;
        self
    }

    pub fn warm_starting(mut self, enabled: bool) -> Self {
        self.config.warm_starting = enabled;
        self
    }

    pub fn contact_recycling(mut self, enabled: bool) -> Self {
        self.config.recycling = enabled;
        self
    }

    pub fn continuous(mut self, enabled: bool) -> Self {
        self.config.continuous = enabled;
        self
    }

    pub fn build(self) -> Result<RigidWorld> {
        let config = self.config;
        if config.substeps == 0 {
            return Err(Error::NoSubsteps);
        }
        for (what, value) in [
            ("gravity", config.gravity.length()),
            ("contact push speed", config.contact_push_speed),
            ("restitution threshold", config.restitution_threshold),
            ("max linear speed", config.max_linear_speed),
        ] {
            if !value.is_finite() {
                return Err(Error::NotFinite { what, value });
            }
        }
        for (what, spring) in [
            ("contact softness", config.contact_softness),
            ("joint softness", config.joint_softness),
        ] {
            if !spring.is_valid() {
                return Err(Error::InvalidSpring { what });
            }
        }
        let simd = self.simd.unwrap_or_else(Simd::detect);
        let executor = self.workers.build()?;
        Ok(RigidWorld::new(config, simd, executor))
    }
}
