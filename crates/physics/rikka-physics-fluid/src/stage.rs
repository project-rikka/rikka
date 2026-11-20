use std::collections::BTreeMap;

use glam::Vec3;
use rikka_gpu::Compute;
use rikka_gpu::{Batch, ComputeBuffer, Ticket};
use rikka_physics_gpu::Ring;
use rikka_renderer_3d::{FLUID_PARTICLE_BYTES as PARTICLE_BYTES, FLUID_SPRAY_BYTES as SPRAY_BYTES};

use crate::body::BodyId;
use crate::config::Config;
use crate::error::Result;
use crate::gpu::settings::{Grid, SettingsBlock};
use crate::gpu::{Bodies, Kernels, Look, SurfaceInputs};
use crate::substance::Substance;
use crate::surface::{Surface, Wetting};

pub(crate) struct Stage {
    pub(crate) compute: Compute,
    pub(crate) kernels: Kernels,
    pub(crate) config: Config,
    pub(crate) gravity: Vec3,
    pub(crate) settings: ComputeBuffer,
    pub(crate) grid: Grid,
    pub(crate) bodies: Bodies,
    pub(crate) look: Look,
    pub(crate) particle_ring: Ring,
    pub(crate) spray_ring: Ring,
    pub(crate) shown: Shown,
    pub(crate) last_spray: (ComputeBuffer, u32),
}

pub(crate) struct Shown {
    pub(crate) surface: Surface,
    pub(crate) wetness: BTreeMap<BodyId, Wetting>,
}

pub(crate) struct Frame {
    particles: ComputeBuffer,
    pub(crate) particle_count: u32,
    spray: ComputeBuffer,
    pub(crate) spray_count: u32,
    wet: Vec<(BodyId, ComputeBuffer)>,
}

impl Stage {
    pub(crate) fn new(
        compute: &Compute,
        config: Config,
        gravity: Vec3,
        prepare: impl FnOnce(&mut Batch),
    ) -> Result<Stage> {
        let kernels = Kernels::new(compute)?;
        let block = config.block(&[Substance::default()], None, gravity);
        let settings = compute.buffer(std::mem::size_of::<SettingsBlock>() as u64)?;
        let look = Look::new(compute, config.capacity, config.grid)?;
        let mut particle_ring = Ring::new(u64::from(config.capacity) * PARTICLE_BYTES);
        let mut spray_ring = Ring::new(u64::from(config.foam_capacity.max(1)) * SPRAY_BYTES);
        let particles = particle_ring.acquire(compute)?;
        let spray = spray_ring.acquire(compute)?;
        let mut batch = compute.batch();
        batch.write(&settings, 0, bytemuck::bytes_of(&block));
        prepare(&mut batch);
        batch.fill(&particles, 0);
        batch.fill(&spray, 0);
        let ready = batch.submit()?;
        Ok(Stage {
            compute: compute.clone(),
            kernels,
            gravity,
            settings,
            grid: config.grid,
            bodies: Bodies::new(compute, config.tri_grid)?,
            look,
            particle_ring,
            spray_ring,
            last_spray: (spray.clone(), 0),
            shown: Shown {
                surface: Surface {
                    particles,
                    particle_count: 0,
                    spray,
                    spray_count: 0,
                    radius: config.radius,
                    ready,
                },
                wetness: BTreeMap::new(),
            },
            config,
        })
    }

    pub(crate) fn surface(&self) -> Surface {
        self.shown.surface.clone()
    }

    pub(crate) fn wetness(&self, body: BodyId) -> Option<Wetting> {
        self.shown.wetness.get(&body).cloned()
    }

    pub(crate) fn render_record(
        &mut self,
        batch: &mut Batch,
        particle_count: u32,
        spray: ComputeBuffer,
        spray_count: u32,
    ) -> Result<Frame> {
        let particles = self.particle_ring.acquire(&self.compute)?;
        self.last_spray = (spray.clone(), spray_count);
        self.look.shape(
            batch,
            &SurfaceInputs {
                kernels: &self.kernels,
                settings: &self.settings,
                grid: self.grid,
                into: &particles,
                count: particle_count,
            },
        );
        let mut wet = Vec::with_capacity(self.bodies.list.len());
        for body in &mut self.bodies.list {
            let buffer = body.wet_ring.acquire(&self.compute)?;
            wet.push((body.id, buffer));
        }
        for (body, (_, buffer)) in self.bodies.list.iter().zip(&wet) {
            self.bodies.wet_out(batch, &self.kernels, body, buffer);
        }
        Ok(Frame {
            particles,
            particle_count,
            spray,
            spray_count,
            wet,
        })
    }

    pub(crate) fn present(&mut self, frame: Frame, ready: &Ticket) {
        let mut wetness = BTreeMap::new();
        for (body, buffer) in frame.wet {
            if self.bodies.body(body).is_ok() {
                wetness.insert(
                    body,
                    Wetting {
                        buffer,
                        ready: ready.clone(),
                    },
                );
            }
        }
        self.shown = Shown {
            surface: Surface {
                particles: frame.particles,
                particle_count: frame.particle_count,
                spray: frame.spray,
                spray_count: frame.spray_count,
                radius: self.config.radius,
                ready: ready.clone(),
            },
            wetness,
        };
    }

    pub(crate) fn present_empty(&mut self) -> Result<()> {
        let mut batch = self.compute.batch();
        let spray = self.spray_ring.acquire(&self.compute)?;
        let frame = self.render_record(&mut batch, 0, spray, 0)?;
        let ready = batch.submit()?;
        self.present(frame, &ready);
        Ok(())
    }
}
