use glam::Vec3;

use super::curve::{Curve, Gradient};
use crate::material::TextureBinding;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ParticleShape {
    #[default]
    Soft,
    Disc,
    Streak,
    Star,
    Petal,
    Leaf,
    Flake,
    Square,
}

impl ParticleShape {
    pub(crate) fn code(self) -> u32 {
        match self {
            Self::Soft => 0,
            Self::Disc => 1,
            Self::Streak => 2,
            Self::Star => 3,
            Self::Petal => 4,
            Self::Leaf => 5,
            Self::Flake => 6,
            Self::Square => 7,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ParticleFacing {
    #[default]
    Camera,
    Velocity,
    Horizontal,
    Axis(Vec3),
    Tumble,
}

impl ParticleFacing {
    pub(crate) fn code(self) -> u32 {
        match self {
            Self::Camera => 0,
            Self::Velocity => 1,
            Self::Horizontal => 2,
            Self::Axis(_) => 3,
            Self::Tumble => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ParticleBlend {
    #[default]
    Alpha,
    Additive,
    Premultiplied,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParticleLighting {
    Unlit,
    Lit { translucency: f32 },
}

impl Default for ParticleLighting {
    fn default() -> Self {
        Self::Lit { translucency: 0.3 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Coloring {
    #[default]
    OverAge,
    PerParticle,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Twinkle {
    pub depth: f32,
    pub rate: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stretch {
    pub seconds: f32,
    pub min: f32,
    pub max: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprite {
    pub texture: TextureBinding,
    pub columns: u32,
    pub rows: u32,
    pub frame_rate: f32,
    pub random_start: bool,
}

impl Sprite {
    pub fn new(texture: TextureBinding) -> Self {
        Self {
            texture,
            columns: 1,
            rows: 1,
            frame_rate: 0.0,
            random_start: false,
        }
    }

    pub fn flipbook(self, columns: u32, rows: u32, frame_rate: f32) -> Self {
        Self {
            columns,
            rows,
            frame_rate,
            ..self
        }
    }

    pub fn frames(&self) -> u32 {
        self.columns.max(1) * self.rows.max(1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Appearance {
    pub shape: ParticleShape,
    pub sprite: Option<Sprite>,
    pub size: Curve,
    pub size_variance: f32,
    pub aspect: f32,
    pub color: Gradient,
    pub coloring: Coloring,
    pub opacity: Curve,
    pub emission: Curve,
    pub twinkle: Twinkle,
    pub rotation: f32,
    pub spin: f32,
    pub spin_variance: f32,
    pub stretch: Stretch,
    pub facing: ParticleFacing,
    pub softness: f32,
    pub blend: ParticleBlend,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            shape: ParticleShape::Soft,
            sprite: None,
            size: Curve::constant(0.05),
            size_variance: 0.0,
            aspect: 1.0,
            color: Gradient::constant(Vec3::ONE),
            coloring: Coloring::OverAge,
            opacity: Curve::keyed([(0.0, 0.0), (0.1, 1.0), (0.8, 1.0), (1.0, 0.0)]),
            emission: Curve::constant(0.0),
            twinkle: Twinkle::default(),
            rotation: 0.0,
            spin: 0.0,
            spin_variance: 0.0,
            stretch: Stretch::default(),
            facing: ParticleFacing::Camera,
            softness: 0.1,
            blend: ParticleBlend::Alpha,
        }
    }
}
