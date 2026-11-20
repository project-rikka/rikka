pub(crate) mod atmosphere;
mod characters;
pub(crate) mod emitter;
mod environment;
mod figure;
pub(crate) mod gpu;
mod panorama;
mod particles;
pub(crate) mod resolve;
mod shadows;
mod sky;
mod space;
pub(crate) mod tier;

pub use characters::{Characters, FACE_LAYER, HAIR_LAYER, SEE_THROUGH_LAYER};
pub use emitter::{
    Appearance, Attractor, Burst, CURVE_KEYS, Coloring, Curve, Emission, Emitter, EmitterShape,
    EmitterTrack, Forces, Gradient, Ground, GroundContact, Launch, MAX_BURST_COUNT, MAX_BURSTS,
    MAX_TRACK_KEYS, ParticleBlend, ParticleFacing, ParticleLighting, ParticleShape, ParticleState,
    STANDARD_GRAVITY, Sprite, Stretch, Turbulence, Twinkle,
};
pub use environment::{Environment, EnvironmentImage};
pub use figure::{Figure, FigureId, Head};
pub use panorama::{Panorama, PanoramaSun};
pub use particles::{ParticleKind, Particles};
pub use resolve::NITS_PER_RADIANCE_UNIT;
pub use shadows::{ScreenShadows, Shadows};
pub use sky::{Fog, Sky};
pub use space::{Lines, Occlusion, Reflections};
pub use tier::{Denoise, Illumination, Resampling, Tier};

use crate::composite::Composite;
use crate::error::Result;
use crate::fluid::LiquidStyle;
use crate::material::Overrides;
use tier::{Mode, Tiers};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Look {
    pub environment: Environment,
    pub fog: Fog,
    pub overrides: Overrides,
    pub shadows: Shadows,
    pub occlusion: Occlusion,
    pub reflections: Reflections,
    pub illumination: Illumination,
    pub denoise: Denoise,
    pub lines: Lines,
    pub particles: Particles,
    pub emitters: Vec<Emitter>,
    pub composite: Composite,
    pub characters: Characters,
    pub liquid: LiquidStyle,
}

impl Look {
    pub(crate) fn tiers(&self, traces: bool) -> Result<Tiers> {
        let gated = |tier: Tier, on: bool| -> Result<Mode> {
            let mode = tier.mode(traces)?;
            Ok(if on { mode } else { Mode::Off })
        };
        Ok(Tiers {
            shadows: self.shadows.tier.mode(traces)?,
            occlusion: gated(self.occlusion.tier, self.occlusion.strength > 0.0)?,
            reflections: gated(self.reflections.tier, self.reflections.strength > 0.0)?,
            illumination: gated(self.illumination.tier, self.illumination.strength > 0.0)?,
        })
    }
}
