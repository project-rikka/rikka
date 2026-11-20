use glam::Vec3;

use crate::look::Tier;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Occlusion {
    pub tier: Tier,
    pub strength: f32,
    pub radius: f32,
    pub samples: u32,
}

impl Default for Occlusion {
    fn default() -> Self {
        Self {
            tier: Tier::Auto,
            strength: 1.0,
            radius: 0.5,
            samples: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reflections {
    pub tier: Tier,
    pub strength: f32,
    pub roughness_limit: f32,
    pub samples: u32,
}

impl Default for Reflections {
    fn default() -> Self {
        Self {
            tier: Tier::Auto,
            strength: 1.0,
            roughness_limit: 0.6,
            samples: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lines {
    pub strength: f32,
    pub width: f32,
    pub color: Vec3,
    pub depth: f32,
    pub crease: f32,
    pub environment: f32,
}

impl Default for Lines {
    fn default() -> Self {
        Self {
            strength: 0.0,
            width: 1.0,
            color: Vec3::new(0.18, 0.12, 0.14),
            depth: 1.0,
            crease: 1.0,
            environment: 0.3,
        }
    }
}
