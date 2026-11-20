use glam::Vec3;

use crate::light::Light;

/// A figure's own take on the frame's leading directional light (the sun or the first
/// directional light).
///
/// Unset fields keep the leading light's values. A figure whose key turns the light gets its
/// shadows from its own figure shadow map rendered along the new direction, and the world's
/// shadows no longer fall on it; one that keeps the direction keeps every shadow.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KeyLight {
    pub direction: Option<Vec3>,
    pub color: Option<Vec3>,
    pub intensity: Option<f32>,
}

/// Lights that shine on one figure only, as cutscene rigs do: a key that replaces the leading
/// light and any number of extra rim or fill lights in world space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LightRig {
    pub key: Option<KeyLight>,
    pub lights: Vec<Light>,
}

impl LightRig {
    pub fn is_empty(&self) -> bool {
        self.key.is_none() && self.lights.is_empty()
    }

    pub(crate) fn keyed(&self, leading: Option<&Light>) -> Option<Light> {
        let key = self.key?;
        let base = match (leading, key.direction) {
            (Some(leading), _) => *leading,
            (None, Some(direction)) => Light::directional(direction),
            (None, None) => return None,
        };
        let mut light = base;
        if let Some(direction) = key.direction {
            light.kind = crate::light::LightKind::Directional {
                direction: direction.normalize_or(Vec3::NEG_Y),
            };
        }
        if let Some(color) = key.color {
            light.color = color;
        }
        if let Some(intensity) = key.intensity {
            light.intensity = intensity;
        }
        Some(light)
    }
}
