use std::fmt::Write;

use rikka_slang::{Bounds, Hint, Literal, ParameterKind, Preset, Role, TextureKind};
use serde::{Deserialize, Serialize};

use crate::types::ValueType;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterDecl {
    pub name: String,
    pub kind: ParameterKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Literal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<Bounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soft_range: Option<Bounds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hints: Vec<Hint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<Preset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
}

impl ParameterDecl {
    pub fn new(name: impl Into<String>, kind: ParameterKind) -> Self {
        Self {
            name: name.into(),
            kind,
            default: None,
            range: None,
            soft_range: None,
            group: None,
            order: None,
            label: None,
            hints: Vec::new(),
            presets: Vec::new(),
            role: None,
        }
    }

    pub fn float(name: impl Into<String>, default: f32) -> Self {
        Self::new(name, ParameterKind::Float).with_default(Literal::Float(default))
    }

    pub fn color(name: impl Into<String>, default: [f32; 3]) -> Self {
        Self::new(
            name,
            ParameterKind::Color {
                alpha: false,
                srgb: false,
            },
        )
        .with_default(Literal::Vec3(default))
    }

    pub fn texture(name: impl Into<String>, kind: TextureKind) -> Self {
        Self::new(
            name,
            ParameterKind::Texture {
                kind,
                sampler: true,
            },
        )
    }

    pub fn with_default(mut self, default: Literal) -> Self {
        self.default = Some(default);
        self
    }

    pub fn with_range(mut self, low: f32, high: f32) -> Self {
        self.range = Some(Bounds { low, high });
        self
    }

    pub fn with_soft_range(mut self, low: f32, high: f32) -> Self {
        self.soft_range = Some(Bounds { low, high });
        self
    }

    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_hint(mut self, hint: Hint) -> Self {
        self.hints.push(hint);
        self
    }

    pub fn with_role(mut self, role: Role) -> Self {
        self.role = Some(role);
        self
    }

    pub fn ty(&self) -> ValueType {
        match self.kind {
            ParameterKind::Float
            | ParameterKind::Int
            | ParameterKind::UInt
            | ParameterKind::Bool
            | ParameterKind::Enum { .. } => ValueType::Float,
            ParameterKind::Vec2 => ValueType::Float2,
            ParameterKind::Vec3 | ParameterKind::Color { alpha: false, .. } => ValueType::Float3,
            ParameterKind::Vec4 | ParameterKind::Color { alpha: true, .. } => ValueType::Float4,
            ParameterKind::Texture { .. } => ValueType::Texture,
        }
    }

    pub(crate) fn slang_type(&self) -> &'static str {
        match self.kind {
            ParameterKind::Int => "int",
            ParameterKind::UInt | ParameterKind::Enum { .. } => "uint",
            ParameterKind::Bool => "bool",
            ParameterKind::Texture { sampler: false, .. } => "uint",
            _ => self.ty().slang(),
        }
    }

    pub(crate) fn read(&self, field: &str) -> String {
        match self.kind {
            ParameterKind::Int | ParameterKind::UInt | ParameterKind::Enum { .. } => {
                format!("float({field})")
            }
            ParameterKind::Bool => format!("({field} ? 1.0 : 0.0)"),
            _ => field.to_owned(),
        }
    }

    pub(crate) fn texts(&self) -> impl Iterator<Item = &str> {
        let options = match &self.kind {
            ParameterKind::Enum { options } => options.as_slice(),
            _ => &[],
        };
        self.group
            .as_deref()
            .into_iter()
            .chain(self.label.as_deref())
            .chain(options.iter().map(String::as_str))
            .chain(self.presets.iter().map(|preset| preset.name.as_str()))
    }

    pub(crate) fn literal_fits(&self, literal: &Literal) -> bool {
        matches!(
            (&self.kind, literal),
            (ParameterKind::Float, Literal::Float(_))
                | (ParameterKind::Vec2, Literal::Vec2(_))
                | (ParameterKind::Vec3, Literal::Vec3(_))
                | (ParameterKind::Vec4, Literal::Vec4(_))
                | (ParameterKind::Color { alpha: false, .. }, Literal::Vec3(_))
                | (ParameterKind::Color { alpha: true, .. }, Literal::Vec4(_))
                | (ParameterKind::Int, Literal::Int(_))
                | (ParameterKind::UInt, Literal::UInt(_))
                | (ParameterKind::Bool, Literal::Bool(_))
                | (ParameterKind::Enum { .. }, Literal::Enum(_))
        )
    }

    pub(crate) fn declaration(&self) -> String {
        let mut line = String::new();
        let mut attribute = |text: String| {
            line.push('[');
            line.push_str(&text);
            line.push_str("] ");
        };
        match &self.kind {
            ParameterKind::Color { srgb, .. } => {
                attribute("Color".to_owned());
                if *srgb {
                    attribute("Srgb".to_owned());
                }
            }
            ParameterKind::Enum { options } => {
                attribute(format!("Options(\"{}\")", options.join("|")));
            }
            ParameterKind::Texture { kind, .. } => {
                attribute(format!("Texture(\"{}\")", texture_kind(*kind)));
            }
            _ => {}
        }
        if let Some(default) = &self.default {
            attribute(format!("Default({})", numbers(default)));
        }
        if let Some(range) = self.range {
            attribute(format!(
                "Range({}, {})",
                float(range.low),
                float(range.high)
            ));
        }
        if let Some(range) = self.soft_range {
            attribute(format!(
                "SoftRange({}, {})",
                float(range.low),
                float(range.high)
            ));
        }
        if let Some(group) = &self.group {
            attribute(format!("Group(\"{group}\")"));
        }
        if let Some(order) = self.order {
            attribute(format!("Order({order})"));
        }
        if let Some(label) = &self.label {
            attribute(format!("Label(\"{label}\")"));
        }
        for hint in &self.hints {
            attribute(format!("Hint(\"{}\")", hint_name(*hint)));
        }
        for preset in &self.presets {
            attribute(format!(
                "Preset(\"{}\", {})",
                preset.name,
                numbers(&preset.value)
            ));
        }
        if let Some(role) = self.role {
            attribute(format!("Role(\"{}\")", role.name()));
        }
        write!(line, "{} {};", self.slang_type(), self.name).expect("writing to a string");
        line
    }
}

pub(crate) fn float(value: f32) -> String {
    format!("{value:?}")
}

fn numbers(literal: &Literal) -> String {
    let values: Vec<f32> = match *literal {
        Literal::Float(value) => vec![value],
        Literal::Vec2(values) => values.to_vec(),
        Literal::Vec3(values) => values.to_vec(),
        Literal::Vec4(values) => values.to_vec(),
        Literal::Int(value) => vec![value as f32],
        Literal::UInt(value) | Literal::Enum(value) => vec![value as f32],
        Literal::Bool(value) => vec![if value { 1.0 } else { 0.0 }],
        Literal::Texture => Vec::new(),
    };
    values.into_iter().map(float).collect::<Vec<_>>().join(", ")
}

fn texture_kind(kind: TextureKind) -> &'static str {
    match kind {
        TextureKind::Colour => "colour",
        TextureKind::Data => "data",
        TextureKind::Normal => "normal",
        TextureKind::Ramp => "ramp",
    }
}

fn hint_name(hint: Hint) -> &'static str {
    match hint {
        Hint::Angle => "angle",
        Hint::Logarithmic => "logarithmic",
        Hint::Hidden => "hidden",
        Hint::Advanced => "advanced",
    }
}
