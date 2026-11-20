mod appearance;
mod curve;
mod emission;
mod evaluate;
mod forces;
pub(crate) mod noise;
mod presets;
mod shape;
pub(crate) mod track;

pub use appearance::{
    Appearance, Coloring, ParticleBlend, ParticleFacing, ParticleLighting, ParticleShape, Sprite,
    Stretch, Twinkle,
};
pub use curve::{CURVE_KEYS, Curve, Gradient};
pub use emission::{Burst, Emission, MAX_BURST_COUNT, MAX_BURSTS};
pub use evaluate::ParticleState;
pub use forces::{Attractor, Forces, Ground, GroundContact, Launch, STANDARD_GRAVITY, Turbulence};
pub use shape::EmitterShape;
pub use track::{EmitterTrack, MAX_TRACK_KEYS};

use glam::Mat4;

use crate::draw::DEFAULT_LAYERS;

#[derive(Clone, Debug, PartialEq)]
pub struct Emitter {
    pub seed: u32,
    pub shape: EmitterShape,
    pub track: EmitterTrack,
    pub emission: Emission,
    pub launch: Launch,
    pub forces: Forces,
    pub ground: Option<Ground>,
    pub appearance: Appearance,
    pub lighting: ParticleLighting,
    pub layers: u32,
    pub bloom: bool,
}

impl Default for Emitter {
    fn default() -> Self {
        Self {
            seed: 0,
            shape: EmitterShape::Point,
            track: EmitterTrack::default(),
            emission: Emission::default(),
            launch: Launch::default(),
            forces: Forces::default(),
            ground: None,
            appearance: Appearance::default(),
            lighting: ParticleLighting::default(),
            layers: DEFAULT_LAYERS,
            bloom: true,
        }
    }
}

impl Emitter {
    pub fn at(self, transform: Mat4) -> Self {
        Self {
            track: self.track.placed(transform),
            ..self
        }
    }

    pub fn seeded(self, seed: u32) -> Self {
        Self { seed, ..self }
    }
}
