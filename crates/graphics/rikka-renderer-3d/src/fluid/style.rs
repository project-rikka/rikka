use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiquidStyle {
    Physical,
    Plain,
    Cel {
        stylization: f32,
        shade_softness: f32,
        shadow_tint: Vec3,
        ambient: f32,
    },
}

impl Default for LiquidStyle {
    fn default() -> Self {
        Self::Cel {
            stylization: 0.85,
            shade_softness: 0.08,
            shadow_tint: Vec3::new(0.88, 0.84, 0.96),
            ambient: 1.0,
        }
    }
}

impl LiquidStyle {
    pub(crate) fn is_stylized(self) -> bool {
        match self {
            Self::Physical => false,
            Self::Plain => true,
            Self::Cel { stylization, .. } => stylization > 0.0,
        }
    }
}
