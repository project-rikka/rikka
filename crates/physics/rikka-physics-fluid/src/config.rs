use glam::Vec3;
use rikka_physics_gpu::REACTION_UNIT;

use crate::ghost::Ghost;
use crate::gpu::settings::{Grid, SettingsBlock};
use crate::lattice::{Lattice, packed};
use crate::substance::{MAX_SUBSTANCES, Substance, SubstanceBlock};

pub(crate) const REACH_PER_SPACING: f32 = 1.7;
const RELAXATION: f32 = 0.05;
const COHESIVE_SAFETY: f32 = 0.5;
const BOND_REACH: f32 = 1.8;
const BOND_SNAP: f32 = 3.0;
const LONGEST_BOND: f32 = 1.0;
const BAND: f32 = 0.3;
const SMOOTHING: f32 = 0.5;
const BULK_RADIUS: f32 = 1.5;
const LONE_RADIUS: f32 = 1.1;
const NARROWEST: f32 = 0.4;
const WIDEST: f32 = 2.5;
const SHAPED_NEIGHBOURS: f32 = 3.0;
const TRAPPED_AIR: [f32; 2] = [0.5, 4.0];
const WAVE_CREST: [f32; 2] = [0.05, 0.5];
const KINETIC: [f32; 2] = [0.1, 2.0];
const AIR_RATE: f32 = 60.0;
const CREST_RATE: f32 = 30.0;
const FOAM_LIFETIME: f32 = 2.0;
const SPRAY_DRAG: f32 = 0.5;
const BUBBLE_LIFT: f32 = 2.0;
const BUBBLE_DRAG: f32 = 3.0;
const SPRAY_BELOW: f32 = 0.2;
const BUBBLE_ABOVE: f32 = 0.8;
pub(crate) const FASTEST_STEP: f32 = 1.0;
const MAX_SWEEPS: f32 = 24.0;
const BOUNDARY_SHARE: f32 = 0.5;
const TENSILE_DEFICIT: f32 = 0.4;
const SURFACE_RELIEF: f32 = 1.0 / 3.0;
const STRETCH_RAMP: f32 = 0.1;
const MOTION_ALLOWANCE: f32 = 0.6;
const PARTICLE_TRAVEL: f32 = 0.5;
const LIQUID_DENSITY: f32 = 1000.0;

#[derive(Clone, Debug)]
pub(crate) struct Config {
    pub(crate) radius: f32,
    pub(crate) spacing: f32,
    pub(crate) h: f32,
    pub(crate) volume: f32,
    pub(crate) capacity: u32,
    pub(crate) foam_capacity: u32,
    pub(crate) floor: Option<f32>,
    pub(crate) bounds: (Vec3, Vec3),
    pub(crate) iterations: u32,
    pub(crate) substeps: (u32, u32),
    pub(crate) cfl: f32,
    pub(crate) thickness: f32,
    pub(crate) lattice: Lattice,
    pub(crate) coupling: f32,
    pub(crate) grid: Grid,
    pub(crate) tri_grid: Grid,
}

impl Config {
    pub(crate) fn band(&self) -> f32 {
        BAND * self.h
    }

    pub(crate) fn tri_cell(&self) -> f32 {
        2.0 * self.h
    }

    pub(crate) fn motion_allowance(&self) -> f32 {
        MOTION_ALLOWANCE * self.h
    }

    pub(crate) fn sweeps(&self, dt: f32, viscosity: f32) -> u32 {
        if viscosity <= 0.0 {
            return 0;
        }
        let strength = dt * viscosity * self.coupling;
        (1.5 * strength.sqrt()).ceil().clamp(2.0, MAX_SWEEPS) as u32
    }

    fn wall_offset(&self) -> f32 {
        self.spacing / (2.0 * std::f32::consts::SQRT_2)
    }

    pub(crate) fn longest_step(&self, substances: &[Substance]) -> f32 {
        let stiffness = substances
            .iter()
            .map(|substance| substance.cohesive_stiffness(self.h))
            .fold(0.0, f32::max);
        if stiffness > 0.0 {
            COHESIVE_SAFETY * 2.0 / stiffness.sqrt()
        } else {
            f32::INFINITY
        }
    }

    pub(crate) fn coupling_of(spacing: f32, h: f32, lattice: &Lattice) -> f32 {
        let softening = 0.01 * h * h;
        let sum: f32 = packed(spacing, h)
            .iter()
            .map(|point| {
                let r = point.length();
                let slope = 2.0 * (1.0 - r / h) / h;
                10.0 * slope * r / (lattice.density * (r * r + softening))
            })
            .sum();
        sum / 3.0
    }

    pub(crate) fn block(
        &self,
        substances: &[Substance],
        tri_grid: Option<Grid>,
        gravity: Vec3,
    ) -> SettingsBlock {
        let h = self.h;
        let inv_s0 = 1.0 / self.lattice.density;
        let stiffness = self.lattice.stiffness * inv_s0 * inv_s0;
        let mut blocks = [SubstanceBlock::default(); MAX_SUBSTANCES];
        for (block, substance) in blocks.iter_mut().zip(substances) {
            *block = substance.block(h, self.spacing);
        }
        let (low, high) = self.bounds;
        let cell = h;
        let tri_cell = self.tri_cell();
        let ghost = Ghost::fitted(self.spacing, h, self.lattice.density, self.wall_offset());
        let (surface, steepest) = ghost.surface_table(self.lattice.density);
        SettingsBlock {
            kernel: [h, 1.0 / h, h * h, inv_s0],
            solver: [
                RELAXATION * stiffness,
                1.0,
                BOND_REACH * self.spacing,
                BOND_SNAP * h,
            ],
            body: [
                self.thickness,
                self.band(),
                self.floor.unwrap_or(0.0),
                if self.floor.is_some() { 1.0 } else { 0.0 },
            ],
            bounds_lo: [low.x, low.y, low.z, self.radius],
            bounds_hi: [high.x, high.y, high.z, LONGEST_BOND * h],
            grid: self.grid.block(true),
            grid_cell: [cell, 1.0 / cell, 0.0, 0.0],
            tri_grid: tri_grid.map_or(self.tri_grid.block(false), |grid| grid.block(true)),
            tri_cell: [
                tri_cell,
                1.0 / tri_cell,
                self.motion_allowance(),
                self.thickness + self.band() + self.motion_allowance() + PARTICLE_TRAVEL * h,
            ],
            look: [
                SMOOTHING,
                1.0 / self.lattice.spread,
                BULK_RADIUS * self.radius,
                LONE_RADIUS * self.radius,
            ],
            look_limits: [NARROWEST, WIDEST, SHAPED_NEIGHBOURS, self.lattice.fullness],
            foam_potential: [TRAPPED_AIR[0], TRAPPED_AIR[1], WAVE_CREST[0], WAVE_CREST[1]],
            foam_energy: [KINETIC[0], KINETIC[1], AIR_RATE, CREST_RATE],
            foam_motion: [FOAM_LIFETIME, SPRAY_DRAG, BUBBLE_LIFT, BUBBLE_DRAG],
            foam_kinds: [
                SPRAY_BELOW * self.lattice.neighbours,
                BUBBLE_ABOVE * self.lattice.neighbours,
                0.0,
                0.0,
            ],
            boundary: [BOUNDARY_SHARE * self.coupling, 0.0, 0.0, 0.0],
            wall_shape: [self.wall_offset(), h / 63.0, substances.len() as f32, 0.0],
            closing: [
                1.0 / steepest,
                TENSILE_DEFICIT,
                SURFACE_RELIEF,
                STRETCH_RAMP,
            ],
            reaction: gravity
                .extend(LIQUID_DENSITY * self.volume / REACTION_UNIT)
                .to_array(),
            wall: ghost.wall_table(),
            substances: blocks,
            surface,
        }
    }
}
