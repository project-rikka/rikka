use glam::Vec3;
use rikka_physics_gpu::words;
use rikka_renderer_3d::FLUID_WET_BYTES as WET_BYTES;

use crate::body::BodyId;
use crate::error::{Error, Result};
use crate::fluid::Fluid;

const DEAD: u32 = 0x8000_0000;
const SUBSTANCE: u32 = 0xff;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub substance: u32,
}

impl Fluid {
    pub fn read_particles(&mut self) -> Result<Vec<Particle>> {
        self.settle()?;
        let slots = u64::from(self.clock.slots);
        if slots == 0 {
            return Ok(Vec::new());
        }
        let state = self.particles.state();
        let mut batch = self.stage.compute.batch();
        let positions = batch.download_range(&state.positions, 0, slots * 16);
        let velocities = batch.download_range(&state.velocities, 0, slots * 16);
        let phases = batch.download_range(&state.phases, 0, slots * 4);
        batch.submit()?;
        let positions: Vec<f32> = words(positions, slots * 16, Error::ShortReadback)?;
        let velocities: Vec<f32> = words(velocities, slots * 16, Error::ShortReadback)?;
        let phases: Vec<u32> = words(phases, slots * 4, Error::ShortReadback)?;
        Ok(phases
            .iter()
            .enumerate()
            .filter(|(_, phase)| *phase & DEAD == 0)
            .map(|(at, phase)| Particle {
                position: Vec3::from_slice(&positions[4 * at..4 * at + 3]),
                velocity: Vec3::from_slice(&velocities[4 * at..4 * at + 3]),
                substance: phase & SUBSTANCE,
            })
            .collect())
    }

    pub fn read_wetness(&self, body: BodyId) -> Result<Vec<[f32; 4]>> {
        let vertices = self.stage.bodies.body(body)?.mesh.vertex_count() as usize;
        let Some(shown) = self.stage.shown.wetness.get(&body) else {
            return Ok(vec![[0.0; 4]; vertices]);
        };
        let mut batch = self.stage.compute.batch();
        let download = batch.download_range(&shown.buffer, 0, vertices as u64 * WET_BYTES);
        batch.submit()?;
        words(download, vertices as u64 * WET_BYTES, Error::ShortReadback)
    }
}
