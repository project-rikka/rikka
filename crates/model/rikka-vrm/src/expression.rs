use rikka_model_core::glam::{Vec2, Vec4};

use rikka_model_core::MaterialId;
use rikka_model_core::NodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExpressionPreset {
    Happy,
    Angry,
    Sad,
    Relaxed,
    Surprised,
    Aa,
    Ih,
    Ou,
    Ee,
    Oh,
    Blink,
    BlinkLeft,
    BlinkRight,
    LookUp,
    LookDown,
    LookLeft,
    LookRight,
    Neutral,
}

impl ExpressionPreset {
    pub const ALL: &'static [Self] = &[
        Self::Happy,
        Self::Angry,
        Self::Sad,
        Self::Relaxed,
        Self::Surprised,
        Self::Aa,
        Self::Ih,
        Self::Ou,
        Self::Ee,
        Self::Oh,
        Self::Blink,
        Self::BlinkLeft,
        Self::BlinkRight,
        Self::LookUp,
        Self::LookDown,
        Self::LookLeft,
        Self::LookRight,
        Self::Neutral,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Happy => "happy",
            Self::Angry => "angry",
            Self::Sad => "sad",
            Self::Relaxed => "relaxed",
            Self::Surprised => "surprised",
            Self::Aa => "aa",
            Self::Ih => "ih",
            Self::Ou => "ou",
            Self::Ee => "ee",
            Self::Oh => "oh",
            Self::Blink => "blink",
            Self::BlinkLeft => "blinkLeft",
            Self::BlinkRight => "blinkRight",
            Self::LookUp => "lookUp",
            Self::LookDown => "lookDown",
            Self::LookLeft => "lookLeft",
            Self::LookRight => "lookRight",
            Self::Neutral => "neutral",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|preset| preset.name() == name)
    }

    #[must_use]
    pub fn from_legacy_name(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "a" => Some(Self::Aa),
            "i" => Some(Self::Ih),
            "u" => Some(Self::Ou),
            "e" => Some(Self::Ee),
            "o" => Some(Self::Oh),
            "blink_l" => Some(Self::BlinkLeft),
            "blink_r" => Some(Self::BlinkRight),
            "joy" => Some(Self::Happy),
            "sorrow" => Some(Self::Sad),
            "fun" => Some(Self::Relaxed),
            other => Self::ALL
                .iter()
                .copied()
                .find(|preset| preset.name().eq_ignore_ascii_case(other)),
        }
    }

    #[must_use]
    pub fn is_blink(self) -> bool {
        matches!(self, Self::Blink | Self::BlinkLeft | Self::BlinkRight)
    }

    #[must_use]
    pub fn is_mouth(self) -> bool {
        matches!(self, Self::Aa | Self::Ih | Self::Ou | Self::Ee | Self::Oh)
    }

    #[must_use]
    pub fn is_look_at(self) -> bool {
        matches!(
            self,
            Self::LookUp | Self::LookDown | Self::LookLeft | Self::LookRight
        )
    }
}

impl std::fmt::Display for ExpressionPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ExpressionName {
    Preset(ExpressionPreset),
    Custom(String),
}

impl ExpressionName {
    #[must_use]
    pub fn preset(&self) -> Option<ExpressionPreset> {
        match self {
            Self::Preset(preset) => Some(*preset),
            Self::Custom(_) => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Preset(preset) => preset.name(),
            Self::Custom(name) => name,
        }
    }
}

impl From<ExpressionPreset> for ExpressionName {
    fn from(preset: ExpressionPreset) -> Self {
        Self::Preset(preset)
    }
}

impl From<String> for ExpressionName {
    fn from(name: String) -> Self {
        match ExpressionPreset::from_name(&name) {
            Some(preset) => Self::Preset(preset),
            None => Self::Custom(name),
        }
    }
}

impl From<&str> for ExpressionName {
    fn from(name: &str) -> Self {
        Self::from(name.to_owned())
    }
}

impl std::fmt::Display for ExpressionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExpressionOverride {
    #[default]
    None,
    Block,
    Blend,
}

named!(ExpressionOverride {
    None => "none",
    Block => "block",
    Blend => "blend",
});

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MorphBind {
    pub node: NodeId,
    pub index: u32,
    pub weight: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MaterialColor {
    Base,
    Emissive,
    Shade,
    Matcap,
    Rim,
    Outline,
}

named!(MaterialColor {
    Base => "color",
    Emissive => "emissionColor",
    Shade => "shadeColor",
    Matcap => "matcapColor",
    Rim => "rimColor",
    Outline => "outlineColor",
});

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialColorBind {
    pub material: MaterialId,
    pub color: MaterialColor,
    pub value: Vec4,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureTransformBind {
    pub material: MaterialId,
    pub scale: Vec2,
    pub offset: Vec2,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expression {
    pub name: ExpressionName,
    pub morph_binds: Vec<MorphBind>,
    pub material_color_binds: Vec<MaterialColorBind>,
    pub texture_transform_binds: Vec<TextureTransformBind>,
    pub is_binary: bool,
    pub override_blink: ExpressionOverride,
    pub override_look_at: ExpressionOverride,
    pub override_mouth: ExpressionOverride,
}

impl Expression {
    pub fn new(name: impl Into<ExpressionName>) -> Self {
        Self {
            name: name.into(),
            morph_binds: Vec::new(),
            material_color_binds: Vec::new(),
            texture_transform_binds: Vec::new(),
            is_binary: false,
            override_blink: ExpressionOverride::None,
            override_look_at: ExpressionOverride::None,
            override_mouth: ExpressionOverride::None,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.morph_binds.is_empty()
            && self.material_color_binds.is_empty()
            && self.texture_transform_binds.is_empty()
    }

    #[must_use]
    pub fn weigh(&self, weight: f32) -> f32 {
        let weight = weight.clamp(0.0, 1.0);
        match self.is_binary {
            true => f32::from(weight > 0.5),
            false => weight,
        }
    }

    #[must_use]
    pub fn is_procedural(&self, kind: Procedural) -> bool {
        self.name.preset().is_some_and(|preset| kind.holds(preset))
    }

    #[must_use]
    pub fn override_of(&self, kind: Procedural) -> ExpressionOverride {
        match kind {
            Procedural::Blink => self.override_blink,
            Procedural::Mouth => self.override_mouth,
            Procedural::LookAt => self.override_look_at,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Procedural {
    Blink,
    Mouth,
    LookAt,
}

impl Procedural {
    pub const ALL: &'static [Self] = &[Self::Blink, Self::Mouth, Self::LookAt];

    #[must_use]
    pub fn holds(self, preset: ExpressionPreset) -> bool {
        match self {
            Self::Blink => preset.is_blink(),
            Self::Mouth => preset.is_mouth(),
            Self::LookAt => preset.is_look_at(),
        }
    }
}
