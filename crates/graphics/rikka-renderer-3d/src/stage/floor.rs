use glam::{Vec2, Vec3};

use crate::error::Result;
use crate::material::{BuiltinTemplate, MaterialDesc, MaterialState, MaterialTemplate, Value};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FloorPattern {
    #[default]
    Plain,
    Tiles,
    Planks,
    Shadow,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloorParams {
    pub color: Vec3,
    pub roughness: f32,
    pub metallic: f32,
    pub reflection: f32,
    pub pattern: FloorPattern,
    pub scale: f32,
    pub contrast: f32,
    pub seam: f32,
    pub seam_color: Vec3,
    pub texture_scale: Vec2,
    pub shadow: f32,
}

impl Default for FloorParams {
    fn default() -> Self {
        Self {
            color: Vec3::new(0.42, 0.4, 0.42),
            roughness: 0.18,
            metallic: 0.0,
            reflection: 0.55,
            pattern: FloorPattern::Plain,
            scale: 1.0,
            contrast: 0.25,
            seam: 0.012,
            seam_color: Vec3::new(0.08, 0.08, 0.09),
            texture_scale: Vec2::ONE,
            shadow: 0.5,
        }
    }
}

impl FloorParams {
    pub fn template(&self) -> MaterialTemplate {
        MaterialTemplate::builtin(match self.pattern {
            FloorPattern::Shadow => BuiltinTemplate::ShadowCatcher,
            _ => BuiltinTemplate::Floor,
        })
    }

    pub fn values(&self) -> Vec<(&'static str, Value)> {
        let color = self.color.max(Vec3::ZERO);
        if self.pattern == FloorPattern::Shadow {
            return vec![
                ("color", color.into()),
                ("shadow", self.shadow.clamp(0.0, 1.0).into()),
            ];
        }
        let (roughness, reflection) = self.gloss();
        vec![
            ("color", color.into()),
            ("roughness", roughness.into()),
            ("metallic", self.metallic.clamp(0.0, 1.0).into()),
            (
                "pattern",
                Value::Enum(
                    match self.pattern {
                        FloorPattern::Planks => "planks",
                        FloorPattern::Tiles => "tiles",
                        FloorPattern::Plain | FloorPattern::Shadow => "plain",
                    }
                    .to_owned(),
                ),
            ),
            ("scale", self.scale.max(1e-3).into()),
            ("contrast", self.contrast.max(0.0).into()),
            ("seam", self.seam.max(0.0).into()),
            ("seam_color", self.seam_color.max(Vec3::ZERO).into()),
            ("texture_scale", self.texture_scale.into()),
            ("gloss.roughness", roughness.into()),
            ("gloss.reflection", reflection.into()),
        ]
    }

    fn gloss(&self) -> (f32, f32) {
        (
            self.roughness.clamp(0.02, 1.0),
            self.reflection.clamp(0.0, 1.0),
        )
    }

    pub fn material(&self) -> Result<MaterialDesc> {
        let state = match self.pattern {
            FloorPattern::Shadow => MaterialState::transparent(),
            _ => MaterialState::opaque(),
        };
        let mut desc = MaterialDesc::new(&self.template()).state(state);
        for (name, value) in self.values() {
            desc.set(name, value)?;
        }
        Ok(desc)
    }
}
