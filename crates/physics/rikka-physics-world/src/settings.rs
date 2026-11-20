use std::sync::Arc;

use rikka_physics_core::Executor;
use rikka_physics_geometry::Tolerance;
use rikka_physics_math::{Simd, Vec3};
use rikka_physics_rigid::RigidWorld;

use crate::Result;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Settings {
    pub gravity: Vec3,
    pub tick_rate: u32,
    pub substeps: u32,
    pub max_ticks: u32,
    pub tolerance: Tolerance,
    pub simd: Simd,
}

impl Settings {
    pub(crate) fn tick_seconds(&self) -> f32 {
        1.0 / self.tick_rate as f32
    }

    pub(crate) fn rigid_world(&self, executor: Arc<dyn Executor>) -> Result<RigidWorld> {
        Ok(RigidWorld::builder()
            .gravity(self.gravity)
            .substeps(self.substeps)
            .tolerance(self.tolerance)
            .simd(self.simd)
            .workers(executor)
            .build()?)
    }
}
