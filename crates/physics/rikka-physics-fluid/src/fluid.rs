use std::collections::BTreeMap;

use glam::Vec3;
use rikka_gpu::Compute;
use rikka_gpu::{ComputeBuffer, Span, Timing};
use rikka_model_core::{Model, NodeId};
use rikka_renderer_3d::DRAPE_VERTEX_BYTES;

use crate::body::{BodyId, BodyMesh, RegionId};
use crate::builder::FluidBuilder;
use crate::config::Config;
use crate::error::{Error, Result};
use crate::frames::{InFlight, Measured, STATS_WORDS};
use crate::gpu::{Foam, Particles, Sorter, SpawnRecords, Viscous};
use crate::stage::Stage;
use crate::step::Carry;
use crate::substance::{MAX_SUBSTANCES, Substance};
use crate::surface::{Surface, Wetting};

pub struct Fluid {
    pub(crate) stage: Stage,
    pub(crate) substances: Vec<Substance>,
    pub(crate) settings_dirty: bool,
    pub(crate) particles: Particles,
    pub(crate) viscous: Viscous,
    pub(crate) sorter: Sorter,
    pub(crate) foam: Foam,
    pub(crate) stats: ComputeBuffer,
    pub(crate) spawn_records: SpawnRecords,
    pub(crate) clock: Clock,
    pub(crate) stats_readback: InFlight,
    pub(crate) profiling: bool,
    pub(crate) timing: Option<Timing>,
    pub(crate) resident_spray: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Clock {
    pub(crate) time: Option<f64>,
    pub(crate) slots: u32,
    pub(crate) spawned: u64,
    pub(crate) measured: Measured,
    pub(crate) steps: u64,
    pub(crate) substeps: u32,
    pub(crate) moved: bool,
    pub(crate) sprayed: bool,
    pub(crate) bonds_active: bool,
    pub(crate) carries: BTreeMap<u32, Carry>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub particles: u32,
    pub spray: u32,
}

impl Fluid {
    pub fn builder() -> FluidBuilder {
        FluidBuilder::default()
    }

    pub(crate) fn new(compute: &Compute, config: Config, gravity: Vec3) -> Result<Fluid> {
        let particles = Particles::new(compute, config.capacity)?;
        let viscous = Viscous::new(compute, config.capacity)?;
        let sorter = Sorter::new(compute, config.capacity, config.grid)?;
        let foam = Foam::new(compute, config.foam_capacity)?;
        let stats = compute.buffer(STATS_WORDS as u64 * 4)?;
        let stage = Stage::new(compute, config, gravity, |batch| {
            batch.fill(&foam.counters, 0);
            viscous.prepare(batch);
        })?;
        Ok(Fluid {
            stage,
            substances: vec![Substance::default()],
            settings_dirty: false,
            particles,
            viscous,
            sorter,
            foam,
            stats,
            spawn_records: SpawnRecords::default(),
            clock: Clock::default(),
            stats_readback: InFlight::default(),
            profiling: false,
            timing: None,
            resident_spray: 0,
        })
    }

    pub fn set_substances(&mut self, substances: &[Substance]) -> Result<()> {
        if substances.is_empty() || substances.len() > MAX_SUBSTANCES {
            return Err(Error::SubstanceCount(substances.len()));
        }
        self.substances = substances.to_vec();
        self.settings_dirty = true;
        Ok(())
    }

    #[must_use]
    pub fn substances(&self) -> &[Substance] {
        &self.substances
    }

    pub fn add_body(&mut self, model: &Model) -> Result<BodyId> {
        let mesh = BodyMesh::flatten(model)?;
        self.settle()?;
        let id = self.stage.bodies.add(&self.stage.compute, mesh)?;
        self.loosen_anchors()?;
        self.settings_dirty = true;
        Ok(id)
    }

    pub fn remove_body(&mut self, body: BodyId) -> Result<()> {
        self.settle()?;
        self.stage.bodies.remove(&self.stage.compute, body)?;
        self.loosen_anchors()?;
        self.stage.shown.wetness.remove(&body);
        self.settings_dirty = true;
        Ok(())
    }

    pub fn add_region(&mut self, body: BodyId, triangles: &[(NodeId, u32)]) -> Result<RegionId> {
        self.stage.bodies.add_region(body, triangles)
    }

    pub fn remove_region(&mut self, region: RegionId) -> bool {
        self.stage.bodies.remove_region(region)
    }

    #[must_use]
    pub fn time(&self) -> Option<f64> {
        self.clock.time
    }

    #[must_use]
    pub fn last_substeps(&self) -> u32 {
        self.clock.substeps
    }

    #[must_use]
    pub fn radius(&self) -> f32 {
        self.stage.config.radius
    }

    #[must_use]
    pub fn gravity(&self) -> Vec3 {
        self.stage.gravity
    }

    #[must_use]
    pub fn particle_volume(&self) -> f32 {
        self.stage.config.volume
    }

    #[must_use]
    pub fn surface(&self) -> Surface {
        self.stage.surface()
    }

    #[must_use]
    pub fn wetness(&self, body: BodyId) -> Option<Wetting> {
        self.stage.wetness(body)
    }

    #[must_use]
    pub fn reaction(&self, body: BodyId, node: NodeId) -> Option<ComputeBuffer> {
        let fluid_body = self.stage.bodies.body(body).ok()?;
        fluid_body
            .reactions
            .iter()
            .find(|(draped, _)| *draped == node)
            .map(|(_, buffer)| buffer.clone())
    }

    pub fn counts(&mut self) -> Result<Counts> {
        self.settle()?;
        Ok(Counts {
            particles: self.clock.measured.latest.live,
            spray: self.clock.measured.latest.foam,
        })
    }

    pub fn set_profiling(&mut self, profiling: bool) {
        self.profiling = profiling;
        if !profiling {
            self.timing = None;
        }
    }

    pub fn profile(&mut self) -> Result<Vec<Span>> {
        match self.timing.take() {
            Some(timing) => Ok(timing.wait()?),
            None => Ok(Vec::new()),
        }
    }

    pub fn reset(&mut self) -> Result<()> {
        self.settle()?;
        self.clock = Clock::default();
        self.resident_spray = 0;
        for body in &mut self.stage.bodies.list {
            body.rest_pose();
        }
        let mut batch = self.stage.compute.batch();
        batch.fill(&self.foam.counters, 0);
        if let Some(buffers) = &self.stage.bodies.buffers {
            batch.fill(&buffers.wet, 0);
            batch.fill(&buffers.deposits, 0);
            batch.fill(&buffers.wet_record, 0);
        }
        batch.submit()?;
        self.stage.present_empty()
    }

    fn loosen_anchors(&self) -> Result<()> {
        let mut batch = self.stage.compute.batch();
        self.particles.loosen(&mut batch);
        batch.submit()?;
        Ok(())
    }

    pub(crate) fn settle(&mut self) -> Result<()> {
        self.stats_readback.settle(&mut self.clock.measured, 0)
    }

    pub(crate) fn validate(&self, inputs: &crate::inputs::Inputs) -> Result<()> {
        for (body, posture) in &inputs.bodies {
            let fluid_body = self.stage.bodies.body(*body)?;
            if posture.palette.len() != fluid_body.mesh.palette as usize {
                return Err(Error::PaletteMismatch {
                    body: *body,
                    expected: fluid_body.mesh.palette as usize,
                    actual: posture.palette.len(),
                });
            }
            if !posture.palette.iter().all(|matrix| matrix.is_finite()) {
                return Err(Error::NonFinitePalette(*body));
            }
            if posture.morph_weights.len() != fluid_body.mesh.morphs.targets() as usize {
                return Err(Error::MorphWeightMismatch {
                    body: *body,
                    expected: fluid_body.mesh.morphs.targets() as usize,
                    actual: posture.morph_weights.len(),
                });
            }
            if !posture
                .morph_weights
                .iter()
                .all(|weight| weight.is_finite())
            {
                return Err(Error::NonFiniteMorphWeight(*body));
            }
        }
        for draped in &inputs.drapes {
            let body = draped.body;
            if !inputs.bodies.iter().any(|(posed, _)| *posed == body) {
                return Err(Error::UnposedDrape(body));
            }
            let fluid_body = self.stage.bodies.body(body)?;
            let Some(part) = fluid_body.mesh.part(draped.node) else {
                return Err(Error::MissingMesh {
                    body,
                    node: draped.node,
                });
            };
            let needed = u64::from(part.vertices) * DRAPE_VERTEX_BYTES;
            if draped.buffer.size() < needed {
                return Err(Error::DrapeTooSmall {
                    body,
                    node: draped.node,
                    size: draped.buffer.size(),
                    needed,
                });
            }
        }
        for emission in &inputs.emissions {
            if emission.substance as usize >= self.substances.len() {
                return Err(Error::UnknownSubstance {
                    index: emission.substance,
                    count: self.substances.len(),
                });
            }
            let finite = emission.from.is_finite()
                && emission.to.is_finite()
                && emission.velocity.is_finite()
                && emission.amount.is_finite();
            if !finite {
                return Err(Error::NonFiniteEmission(emission.emitter));
            }
            if let crate::emission::Nozzle::Surface { region, .. } = emission.nozzle
                && self.stage.bodies.region(region).is_none()
            {
                return Err(Error::UnknownRegion(emission.emitter));
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for Fluid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fluid")
            .field("radius", &self.stage.config.radius)
            .field("capacity", &self.stage.config.capacity)
            .field("time", &self.clock.time)
            .field("slots", &self.clock.slots)
            .finish_non_exhaustive()
    }
}
