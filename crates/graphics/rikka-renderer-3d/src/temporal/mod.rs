mod declare;
mod jitter;
mod scale;
mod state;
mod timer;

use std::time::Duration;

pub(crate) use declare::{KERNELS, Tonal};
pub(crate) use state::{TemporalFrame, TemporalState};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Antialiasing {
    #[default]
    Multisample,
    Temporal,
    Upscale(Upscale),
}

impl Antialiasing {
    pub fn is_temporal(&self) -> bool {
        !matches!(self, Self::Multisample)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Upscale {
    pub scale: RenderScale,
    pub sharpness: f32,
}

impl Upscale {
    pub const DEFAULT_SHARPNESS: f32 = 0.5;

    pub const fn fixed(scale: f32) -> Self {
        Self {
            scale: RenderScale::Fixed(scale),
            sharpness: Self::DEFAULT_SHARPNESS,
        }
    }

    pub fn dynamic(target: Duration) -> Self {
        Self {
            scale: RenderScale::Dynamic(DynamicScale::new(target)),
            sharpness: Self::DEFAULT_SHARPNESS,
        }
    }

    pub const fn sharpness(mut self, sharpness: f32) -> Self {
        self.sharpness = sharpness;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RenderScale {
    Fixed(f32),
    Dynamic(DynamicScale),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DynamicScale {
    pub target: Duration,
    pub min: f32,
    pub max: f32,
}

impl DynamicScale {
    pub const MIN: f32 = 0.5;
    pub const MAX: f32 = 1.0;

    pub fn new(target: Duration) -> Self {
        Self {
            target,
            min: Self::MIN,
            max: Self::MAX,
        }
    }

    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = min;
        self.max = max;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Resolution {
    Render,
    Display,
}

pub(crate) const SCALE_MIN: f32 = 0.25;

pub(crate) fn clamp_scale(scale: f32) -> f32 {
    match scale.is_finite() {
        true => scale.clamp(SCALE_MIN, 1.0),
        false => 1.0,
    }
}
