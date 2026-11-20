use glam::{Vec2, Vec3};

use crate::error::Result;
use crate::material::{BuiltinTemplate, MaterialDesc, MaterialState, MaterialTemplate, Value};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterParams {
    pub deep: Vec3,
    pub shallow: Vec3,
    pub height: f32,
    pub scale: f32,
    pub speed: f32,
    pub glint: f32,
    pub direction: Vec2,
    pub roughness: f32,
    pub reflection: f32,
    pub reflection_blur: f32,
}

impl Default for WaterParams {
    fn default() -> Self {
        Self {
            deep: Vec3::new(0.004, 0.018, 0.03),
            shallow: Vec3::new(0.03, 0.16, 0.2),
            height: 1.0,
            scale: 4.0,
            speed: 1.0,
            glint: 1.0,
            direction: Vec2::new(1.0, 0.35),
            roughness: 0.05,
            reflection: 0.14,
            reflection_blur: 0.12,
        }
    }
}

impl WaterParams {
    pub fn values(&self) -> Vec<(&'static str, Value)> {
        vec![
            ("deep", self.deep.max(Vec3::ZERO).into()),
            ("height", self.height.max(0.0).into()),
            ("shallow", self.shallow.max(Vec3::ZERO).into()),
            ("scale", self.scale.max(1e-3).into()),
            ("speed", self.speed.into()),
            ("glint", self.glint.max(0.0).into()),
            ("direction", self.direction.into()),
            ("roughness", self.roughness.clamp(0.02, 1.0).into()),
            (
                "gloss.roughness",
                self.reflection_blur.clamp(0.0, 1.0).into(),
            ),
            ("gloss.reflection", self.reflection.clamp(0.0, 1.0).into()),
        ]
    }

    pub fn material(&self) -> Result<MaterialDesc> {
        let mut desc = MaterialDesc::new(&MaterialTemplate::builtin(BuiltinTemplate::Water))
            .state(MaterialState::opaque());
        for (name, value) in self.values() {
            desc.set(name, value)?;
        }
        Ok(desc)
    }
}
