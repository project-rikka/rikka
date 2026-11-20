use glam::Vec3;
use rikka_gpu::Compute;
use rikka_physics_tailor::Quality;

use crate::cloth::Cloth;
use crate::error::{Error, Result};

pub(crate) const MIN_RATE: f32 = 15.0;
pub(crate) const MAX_RATE: f32 = 480.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Effort {
    pub(crate) substeps: (u32, u32),
    pub(crate) epochs: u32,
}

impl From<Quality> for Effort {
    fn from(quality: Quality) -> Effort {
        match quality {
            Quality::Low => Effort {
                substeps: (3, 6),
                epochs: 1,
            },
            Quality::Medium => Effort {
                substeps: (6, 10),
                epochs: 1,
            },
            Quality::High => Effort {
                substeps: (12, 24),
                epochs: 2,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Settings {
    pub(crate) gravity: Vec3,
    pub(crate) rate: f32,
    pub(crate) floor: Option<f32>,
    pub(crate) effort: Effort,
}

impl Settings {
    pub(crate) fn epoch_span(&self) -> f32 {
        1.0 / (self.rate * self.effort.epochs as f32)
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClothBuilder {
    settings: Settings,
}

impl Default for ClothBuilder {
    fn default() -> ClothBuilder {
        ClothBuilder {
            settings: Settings {
                gravity: Vec3::new(0.0, -9.81, 0.0),
                rate: 60.0,
                floor: None,
                effort: Effort::from(Quality::Medium),
            },
        }
    }
}

impl ClothBuilder {
    pub fn gravity(mut self, gravity: Vec3) -> ClothBuilder {
        self.settings.gravity = gravity;
        self
    }

    pub fn rate(mut self, hertz: f32) -> ClothBuilder {
        self.settings.rate = hertz;
        self
    }

    pub fn floor(mut self, height: impl Into<Option<f32>>) -> ClothBuilder {
        self.settings.floor = height.into();
        self
    }

    pub fn quality(mut self, quality: Quality) -> ClothBuilder {
        self.settings.effort = Effort::from(quality);
        self
    }

    pub fn build(self, compute: &Compute) -> Result<Cloth> {
        let settings = self.settings;
        if !settings.gravity.is_finite() {
            return Err(Error::NonFinite("gravity"));
        }
        if !(MIN_RATE..=MAX_RATE).contains(&settings.rate) {
            return Err(Error::RateOutOfRange(settings.rate));
        }
        if settings.floor.is_some_and(|floor| !floor.is_finite()) {
            return Err(Error::NonFinite("floor height"));
        }
        Cloth::new(compute, settings)
    }
}
