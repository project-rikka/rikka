use glam::Vec3;

use crate::id::model_id;

model_id!(LightId);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LightKind {
    Directional,
    Point {
        range: f32,
    },
    Spot {
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
    },
}

impl LightKind {
    #[must_use]
    pub fn scaled(self, factor: f32) -> Self {
        match self {
            Self::Directional => Self::Directional,
            Self::Point { range } => Self::Point {
                range: range * factor,
            },
            Self::Spot {
                range,
                inner_angle,
                outer_angle,
            } => Self::Spot {
                range: range * factor,
                inner_angle,
                outer_angle,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    pub name: String,
    pub kind: LightKind,
    pub color: Vec3,
    pub intensity: f32,
}

impl Light {
    pub fn new(name: impl Into<String>, kind: LightKind) -> Self {
        Self {
            name: name.into(),
            kind,
            color: Vec3::ONE,
            intensity: 1.0,
        }
    }

    #[must_use]
    pub fn color(mut self, color: Vec3) -> Self {
        self.color = color;
        self
    }

    #[must_use]
    pub fn intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity;
        self
    }
}
