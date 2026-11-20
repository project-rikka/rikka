use std::ops::RangeInclusive;

use rikka_model_core::glam::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Gravity {
    #[default]
    Earth,
    Custom(Vec3),
}

impl Gravity {
    pub const EARTH: f32 = 9.81;

    #[must_use]
    pub fn vector(self) -> Vec3 {
        match self {
            Self::Earth => Vec3::new(0.0, -Self::EARTH, 0.0),
            Self::Custom(acceleration) => acceleration,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Wetness {
    Off,
    On { drying: f32 },
}

impl Default for Wetness {
    fn default() -> Self {
        Self::On {
            drying: FluidSettings::DRYING,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidSettings {
    pub particle_size: f32,
    pub capacity: u32,
    pub wetness: Wetness,
    pub gravity: Gravity,
}

impl FluidSettings {
    pub const PARTICLE_SIZES: RangeInclusive<f32> = 0.002..=0.05;
    pub const CAPACITIES: RangeInclusive<u32> = 10_000..=4_000_000;
    pub const DRYING_SECONDS: RangeInclusive<f32> = 1.0..=600.0;
    pub const DRYING: f32 = 30.0;

    #[must_use]
    pub fn radius(&self) -> f32 {
        self.particle_size * 0.5
    }

    #[must_use]
    pub fn drying_rate(&self) -> f32 {
        match self.wetness {
            Wetness::On { drying } => drying.max(f32::EPSILON).recip(),
            Wetness::Off => 0.0,
        }
    }
}

impl Default for FluidSettings {
    fn default() -> Self {
        Self {
            particle_size: 0.004,
            capacity: 500_000,
            wetness: Wetness::default(),
            gravity: Gravity::Earth,
        }
    }
}
