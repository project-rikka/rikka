use crate::error::{Error, Result};
use rikka_gpu::error::Capability;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Tier {
    Off,
    ScreenSpace,
    RayTraced,
    #[default]
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Mode {
    Off,
    ScreenSpace,
    RayTraced,
}

impl Tier {
    pub(crate) fn mode(self, traces: bool) -> Result<Mode> {
        match (self, traces) {
            (Self::Off, _) => Ok(Mode::Off),
            (Self::ScreenSpace, _) | (Self::Auto, false) => Ok(Mode::ScreenSpace),
            (Self::RayTraced | Self::Auto, true) => Ok(Mode::RayTraced),
            (Self::RayTraced, false) => Err(Error::not_enabled(Capability::RayTracing)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Resampling {
    Off,
    Temporal,
    Spatial,
    #[default]
    Spatiotemporal,
}

impl Resampling {
    pub(crate) fn temporal(self) -> bool {
        matches!(self, Self::Temporal | Self::Spatiotemporal)
    }

    pub(crate) fn spatial(self) -> bool {
        matches!(self, Self::Spatial | Self::Spatiotemporal)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Illumination {
    pub tier: Tier,
    pub strength: f32,
    pub reach: f32,
    pub samples: u32,
    pub resampling: Resampling,
}

impl Default for Illumination {
    fn default() -> Self {
        Self {
            tier: Tier::Auto,
            strength: 1.0,
            reach: 12.0,
            samples: 1,
            resampling: Resampling::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Denoise {
    pub history: f32,
    pub passes: u32,
}

impl Default for Denoise {
    fn default() -> Self {
        Self {
            history: 32.0,
            passes: 3,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Tiers {
    pub shadows: Mode,
    pub occlusion: Mode,
    pub reflections: Mode,
    pub illumination: Mode,
}

impl Tiers {
    pub(crate) const NONE: Self = Self {
        shadows: Mode::ScreenSpace,
        occlusion: Mode::Off,
        reflections: Mode::Off,
        illumination: Mode::Off,
    };

    pub(crate) fn traces(self) -> bool {
        [
            self.shadows,
            self.occlusion,
            self.reflections,
            self.illumination,
        ]
        .contains(&Mode::RayTraced)
    }

    pub(crate) fn traces_scene(self, shadow_lights: bool) -> bool {
        (self.shadows == Mode::RayTraced && shadow_lights)
            || [self.occlusion, self.reflections, self.illumination].contains(&Mode::RayTraced)
    }
}
