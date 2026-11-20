use glam::Vec3;
use rikka_gpu::Compute;
use rikka_physics_gpu::MAX_SCANNED;

use crate::config::{Config, REACH_PER_SPACING};
use crate::error::{Error, Result};
use crate::fluid::Fluid;
use crate::gpu::settings::{Grid, NARROWEST_GRID};
use crate::lattice::Lattice;
use crate::replay::Replay;

pub(crate) const MIN_RADIUS: f32 = 1.0e-4;
pub(crate) const MAX_RADIUS: f32 = 0.1;
pub(crate) const BINS_PER_PARTICLE: u32 = 8;
const MIN_BINS: u32 = 1 << 15;
const MAX_BINS: u32 = 1 << 21;
const TRIANGLE_BINS: u32 = 1 << 19;
const NARROWEST_TRIANGLE_GRID: u32 = 6;
const _: () = assert!(MAX_BINS + 2 <= MAX_SCANNED && TRIANGLE_BINS + 2 <= MAX_SCANNED);
const BOUNDS_LOW: Vec3 = Vec3::new(-10.0, -5.0, -10.0);
const BOUNDS_HIGH: Vec3 = Vec3::new(10.0, 15.0, 10.0);
const CFL: f32 = 0.5;
const BODY_OFFSET_PER_RADIUS: f32 = 0.25;

#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct FluidBuilder {
    radius: f32,
    capacity: u32,
    spray_capacity: Option<u32>,
    gravity: Vec3,
    floor: Option<f32>,
    iterations: u32,
    substeps: (u32, u32),
}

impl Default for FluidBuilder {
    fn default() -> Self {
        Self {
            radius: 0.002,
            capacity: 1 << 18,
            spray_capacity: None,
            gravity: Vec3::new(0.0, -9.81, 0.0),
            floor: None,
            iterations: 3,
            substeps: (2, 16),
        }
    }
}

impl FluidBuilder {
    pub fn radius(mut self, metres: f32) -> FluidBuilder {
        self.radius = metres;
        self
    }

    pub fn capacity(mut self, particles: u32) -> FluidBuilder {
        self.capacity = particles;
        self
    }

    pub fn spray_capacity(mut self, particles: u32) -> FluidBuilder {
        self.spray_capacity = Some(particles);
        self
    }

    pub fn gravity(mut self, gravity: Vec3) -> FluidBuilder {
        self.gravity = gravity;
        self
    }

    pub fn floor(mut self, height: impl Into<Option<f32>>) -> FluidBuilder {
        self.floor = height.into();
        self
    }

    pub fn iterations(mut self, count: u32) -> FluidBuilder {
        self.iterations = count;
        self
    }

    pub fn substeps(mut self, fewest: u32, most: u32) -> FluidBuilder {
        self.substeps = (fewest, most);
        self
    }

    pub fn build(self, compute: &Compute) -> Result<Fluid> {
        let config = self.config()?;
        Fluid::new(compute, config, self.gravity)
    }

    pub fn replay(self, compute: &Compute) -> Result<Replay> {
        let config = self.config()?;
        Replay::new(compute, config, self.gravity)
    }

    fn config(&self) -> Result<Config> {
        if !(MIN_RADIUS..=MAX_RADIUS).contains(&self.radius) {
            return Err(Error::RadiusOutOfRange(self.radius));
        }
        if self.capacity == 0 || self.capacity >= MAX_SCANNED {
            return Err(Error::CapacityOutOfRange(self.capacity));
        }
        let foam_capacity = self.spray_capacity.unwrap_or(self.capacity / 4);
        if foam_capacity >= MAX_SCANNED {
            return Err(Error::SprayCapacityOutOfRange(foam_capacity));
        }
        if !self.gravity.is_finite() {
            return Err(Error::NonFinite("gravity"));
        }
        if self.iterations == 0 || self.substeps.0 > self.substeps.1 || self.substeps.1 == 0 {
            return Err(Error::InvalidIterations);
        }
        if self.floor.is_some_and(|floor| !floor.is_finite()) {
            return Err(Error::NonFinite("floor height"));
        }
        let spacing = 2.0 * self.radius;
        let h = REACH_PER_SPACING * spacing;
        let lattice = Lattice::measure(spacing, h);
        let bins = self
            .capacity
            .saturating_mul(BINS_PER_PARTICLE)
            .clamp(MIN_BINS, MAX_BINS);
        Ok(Config {
            radius: self.radius,
            spacing,
            h,
            volume: spacing * spacing * spacing / std::f32::consts::SQRT_2,
            capacity: self.capacity,
            foam_capacity,
            floor: self.floor,
            bounds: (BOUNDS_LOW, BOUNDS_HIGH),
            iterations: self.iterations,
            substeps: self.substeps,
            cfl: CFL,
            thickness: self.radius + BODY_OFFSET_PER_RADIUS * self.radius,
            coupling: Config::coupling_of(spacing, h, &lattice),
            lattice,
            grid: Grid::sized(bins, NARROWEST_GRID),
            tri_grid: Grid::sized(TRIANGLE_BINS, NARROWEST_TRIANGLE_GRID),
        })
    }
}
