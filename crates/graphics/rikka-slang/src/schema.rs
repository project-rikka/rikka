use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::layout::{Argument, Attribute, Field, Scalar, Type, TypeLayout};

pub const TEXTURE_TYPE: &str = "RikkaTexture";

const TEXTURE_BYTES: usize = 8;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ParameterSchema {
    parameters: Vec<ParameterSpec>,
    size: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParameterSpec {
    pub name: String,
    pub kind: ParameterKind,
    pub offset: usize,
    pub default: Literal,
    pub range: Option<Bounds>,
    pub soft_range: Option<Bounds>,
    pub group: Option<String>,
    pub order: i32,
    pub label: Option<String>,
    pub hints: Vec<Hint>,
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub feature: Option<u32>,
    pub role: Option<Role>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterKind {
    Float,
    Vec2,
    Vec3,
    Vec4,
    Color { alpha: bool, srgb: bool },
    Int,
    UInt,
    Bool,
    Enum { options: Vec<String> },
    Texture { kind: TextureKind, sampler: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextureKind {
    Colour,
    Data,
    Normal,
    Ramp,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Literal {
    Float(f32),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Int(i32),
    UInt(u32),
    Bool(bool),
    Enum(u32),
    Texture,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bounds {
    pub low: f32,
    pub high: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hint {
    Angle,
    Logarithmic,
    Hidden,
    Advanced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Albedo,
    AlbedoTexture,
    AlbedoWeight,
    Metalness,
    MetalnessTexture,
    Transmission,
    Subsurface,
    Emission,
    EmissionTexture,
    EmissionLuminance,
    Displacement,
}

impl Role {
    pub const ALL: [Self; 11] = [
        Self::Albedo,
        Self::AlbedoTexture,
        Self::AlbedoWeight,
        Self::Metalness,
        Self::MetalnessTexture,
        Self::Transmission,
        Self::Subsurface,
        Self::Emission,
        Self::EmissionTexture,
        Self::EmissionLuminance,
        Self::Displacement,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Albedo => "albedo",
            Self::AlbedoTexture => "albedo_texture",
            Self::AlbedoWeight => "albedo_weight",
            Self::Metalness => "metalness",
            Self::MetalnessTexture => "metalness_texture",
            Self::Transmission => "transmission",
            Self::Subsurface => "subsurface",
            Self::Emission => "emission",
            Self::EmissionTexture => "emission_texture",
            Self::EmissionLuminance => "emission_luminance",
            Self::Displacement => "displacement",
        }
    }

    pub fn fits(self, kind: &ParameterKind) -> bool {
        match self {
            Self::Albedo | Self::Emission => matches!(
                kind,
                ParameterKind::Vec3 | ParameterKind::Vec4 | ParameterKind::Color { .. }
            ),
            Self::AlbedoWeight
            | Self::Metalness
            | Self::Transmission
            | Self::Subsurface
            | Self::EmissionLuminance
            | Self::Displacement => matches!(kind, ParameterKind::Float),
            Self::AlbedoTexture | Self::MetalnessTexture | Self::EmissionTexture => {
                matches!(kind, ParameterKind::Texture { sampler: true, .. })
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub value: Literal,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    #[error("parameter {name} has unsupported type {ty}")]
    Unsupported { name: String, ty: String },
    #[error("parameter {name}: {reason}")]
    Attribute { name: String, reason: String },
    #[error("parameter {0} is declared twice")]
    Duplicate(String),
    #[error("{0} is not a struct")]
    NotStruct(String),
}

impl ParameterSchema {
    pub fn from_struct(layout: &TypeLayout, offset: usize) -> Result<Self, SchemaError> {
        let Type::Struct { name, fields } = &layout.ty else {
            return Err(SchemaError::NotStruct(format!("{:?}", layout.ty)));
        };
        let mut schema = Self {
            parameters: Vec::new(),
            size: offset + layout.size,
        };
        let context = Inherited {
            prefix: String::new(),
            offset,
            group: None,
        };
        collect(fields, &context, &mut schema.parameters).map_err(|error| match error {
            SchemaError::NotStruct(_) => SchemaError::NotStruct(name.clone()),
            other => other,
        })?;
        schema.check_unique()?;
        Ok(schema)
    }

    pub fn merge(mut self, other: Self) -> Result<Self, SchemaError> {
        self.size = self.size.max(other.size);
        let shift = self.parameters.len() as i32;
        self.parameters
            .extend(other.parameters.into_iter().map(|parameter| ParameterSpec {
                order: parameter.order + shift,
                ..parameter
            }));
        self.check_unique()?;
        Ok(self)
    }

    fn check_unique(&self) -> Result<(), SchemaError> {
        let mut seen = BTreeSet::new();
        match self
            .parameters
            .iter()
            .find(|parameter| !seen.insert(parameter.name.as_str()))
        {
            Some(parameter) => Err(SchemaError::Duplicate(parameter.name.clone())),
            None => Ok(()),
        }
    }

    pub fn parameters(&self) -> &[ParameterSpec] {
        &self.parameters
    }

    pub fn parameter(&self, name: &str) -> Option<&ParameterSpec> {
        self.parameters
            .iter()
            .find(|parameter| parameter.name == name)
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.parameters
            .iter()
            .position(|parameter| parameter.name == name)
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn with_role(&self, role: Role) -> Option<&ParameterSpec> {
        self.parameters
            .iter()
            .find(|parameter| parameter.role == Some(role))
    }

    pub fn presets(&self) -> BTreeSet<&str> {
        self.parameters
            .iter()
            .flat_map(|parameter| &parameter.presets)
            .map(|preset| preset.name.as_str())
            .collect()
    }
}

impl ParameterSpec {
    pub fn preset(&self, name: &str) -> Option<Literal> {
        self.presets
            .iter()
            .find(|preset| preset.name == name)
            .map(|preset| preset.value)
    }
}

impl ParameterKind {
    pub fn components(&self) -> usize {
        match self {
            Self::Float | Self::Int | Self::UInt | Self::Bool | Self::Enum { .. } => 1,
            Self::Vec2 => 2,
            Self::Vec3 | Self::Color { alpha: false, .. } => 3,
            Self::Vec4 | Self::Color { alpha: true, .. } => 4,
            Self::Texture { .. } => 0,
        }
    }

    pub fn is_numeric(&self) -> bool {
        !matches!(self, Self::Texture { .. } | Self::Bool | Self::Enum { .. })
    }
}

struct Inherited {
    prefix: String,
    offset: usize,
    group: Option<String>,
}

fn collect(
    fields: &[Field],
    context: &Inherited,
    out: &mut Vec<ParameterSpec>,
) -> Result<(), SchemaError> {
    for field in fields {
        let name = format!("{}{}", context.prefix, field.name);
        let offset = context.offset + field.offset;
        let group = text(field, "Group", &name)?.or_else(|| context.group.clone());
        if let Type::Struct { name: ty, fields } = &field.layout.ty
            && ty != TEXTURE_TYPE
        {
            let nested = Inherited {
                prefix: format!("{name}."),
                offset,
                group,
            };
            collect(fields, &nested, out)?;
            continue;
        }
        let kind = kind_of(field, &name)?;
        if matches!(kind, ParameterKind::Texture { sampler: true, .. })
            && field.layout.size != TEXTURE_BYTES
        {
            return Err(SchemaError::Unsupported {
                name,
                ty: TEXTURE_TYPE.to_owned(),
            });
        }
        let default = match field.attribute("Default") {
            Some(attribute) => literal(&kind, &attribute.arguments, &name)?,
            None => zero(&kind),
        };
        let parameter = ParameterSpec {
            order: match field.attribute("Order") {
                Some(attribute) => number(attribute, 0, &name)? as i32,
                None => out.len() as i32,
            },
            range: bounds(field, "Range", &kind, &name)?,
            soft_range: bounds(field, "SoftRange", &kind, &name)?,
            label: text(field, "Label", &name)?,
            feature: feature(field, &name)?,
            hints: hints(field, &name)?,
            presets: presets(field, &kind, &name)?,
            role: role(field, &kind, &name)?,
            name,
            kind,
            offset,
            default,
            group,
        };
        out.push(parameter);
    }
    Ok(())
}

fn kind_of(field: &Field, name: &str) -> Result<ParameterKind, SchemaError> {
    let unsupported = || SchemaError::Unsupported {
        name: name.to_owned(),
        ty: describe(&field.layout.ty),
    };
    let color = field.attribute("Color").is_some();
    let srgb = field.attribute("Srgb").is_some();
    let options = text(field, "Options", name)?;
    let texture = match field.attribute("Texture") {
        None => None,
        Some(_) => Some(match text(field, "Texture", name)?.as_deref() {
            None | Some("" | "colour") => TextureKind::Colour,
            Some("data") => TextureKind::Data,
            Some("normal") => TextureKind::Normal,
            Some("ramp") => TextureKind::Ramp,
            Some(other) => {
                return Err(attribute_error(
                    name,
                    format!("unknown texture kind {other}"),
                ));
            }
        }),
    };
    let kind = match (&field.layout.ty, color, options) {
        (Type::Struct { name: ty, .. }, false, None) if ty == TEXTURE_TYPE => {
            ParameterKind::Texture {
                kind: texture.unwrap_or(TextureKind::Colour),
                sampler: true,
            }
        }
        (Type::Scalar(Scalar::U32), false, None) if texture.is_some() => ParameterKind::Texture {
            kind: texture.unwrap_or(TextureKind::Colour),
            sampler: false,
        },
        (Type::Scalar(Scalar::F32), false, None) => ParameterKind::Float,
        (Type::Scalar(Scalar::I32), false, None) => ParameterKind::Int,
        (Type::Scalar(Scalar::U32), false, None) => ParameterKind::UInt,
        (Type::Scalar(Scalar::I32 | Scalar::U32), false, Some(options)) => ParameterKind::Enum {
            options: options.split('|').map(str::to_owned).collect(),
        },
        (Type::Scalar(Scalar::Bool), false, None) => ParameterKind::Bool,
        (
            Type::Vector {
                scalar: Scalar::F32,
                count: 2,
            },
            false,
            None,
        ) => ParameterKind::Vec2,
        (
            Type::Vector {
                scalar: Scalar::F32,
                count: 3,
            },
            false,
            None,
        ) => ParameterKind::Vec3,
        (
            Type::Vector {
                scalar: Scalar::F32,
                count: 4,
            },
            false,
            None,
        ) => ParameterKind::Vec4,
        (
            Type::Vector {
                scalar: Scalar::F32,
                count: count @ (3 | 4),
            },
            true,
            None,
        ) => ParameterKind::Color {
            alpha: *count == 4,
            srgb,
        },
        (_, true, _) => {
            return Err(attribute_error(
                name,
                "Color needs a float3 or float4".into(),
            ));
        }
        (_, _, Some(_)) => {
            return Err(attribute_error(name, "Options needs an int or uint".into()));
        }
        _ => return Err(unsupported()),
    };
    if srgb && !color {
        return Err(attribute_error(name, "Srgb needs Color".into()));
    }
    if texture.is_some() && !matches!(kind, ParameterKind::Texture { .. }) {
        return Err(attribute_error(
            name,
            format!("Texture needs a {TEXTURE_TYPE} or a uint index"),
        ));
    }
    Ok(kind)
}

fn describe(ty: &Type) -> String {
    match ty {
        Type::Scalar(scalar) => format!("{scalar:?}"),
        Type::Vector { scalar, count } => format!("{scalar:?}x{count}"),
        Type::Matrix {
            scalar,
            rows,
            columns,
        } => format!("{scalar:?}{rows}x{columns}"),
        Type::Array { .. } => "array".to_owned(),
        Type::Struct { name, .. } => name.clone(),
        Type::Pointer { pointee } => format!("{pointee}*"),
        Type::ConstantBuffer(_) => "constant buffer".to_owned(),
        Type::Resource { .. } => "resource".to_owned(),
        Type::Sampler => "sampler".to_owned(),
        Type::Other(name) => name.clone(),
    }
}

fn attribute_error(name: &str, reason: String) -> SchemaError {
    SchemaError::Attribute {
        name: name.to_owned(),
        reason,
    }
}

fn text(field: &Field, attribute: &str, name: &str) -> Result<Option<String>, SchemaError> {
    let Some(found) = field.attribute(attribute) else {
        return Ok(None);
    };
    match found.arguments.first().and_then(Argument::as_str) {
        Some(value) => Ok(Some(value.to_owned())),
        None => Err(attribute_error(name, format!("{attribute} needs a string"))),
    }
}

fn number(attribute: &Attribute, index: usize, name: &str) -> Result<f32, SchemaError> {
    attribute
        .arguments
        .get(index)
        .and_then(Argument::as_f32)
        .ok_or_else(|| {
            attribute_error(
                name,
                format!("{} needs a number at {index}", attribute.name),
            )
        })
}

fn numbers(arguments: &[Argument], count: usize, name: &str) -> Result<Vec<f32>, SchemaError> {
    let values: Vec<f32> = arguments.iter().filter_map(Argument::as_f32).collect();
    if values.len() < count {
        return Err(attribute_error(
            name,
            format!("value needs {count} numbers"),
        ));
    }
    Ok(values[..count].to_vec())
}

fn literal(
    kind: &ParameterKind,
    arguments: &[Argument],
    name: &str,
) -> Result<Literal, SchemaError> {
    let values = numbers(arguments, kind.components().max(1), name)?;
    let whole = |value: f32| -> Result<i32, SchemaError> {
        match value.fract() == 0.0 {
            true => Ok(value as i32),
            false => Err(attribute_error(
                name,
                format!("{value} is not a whole number"),
            )),
        }
    };
    Ok(match kind {
        ParameterKind::Float => Literal::Float(values[0]),
        ParameterKind::Vec2 => Literal::Vec2([values[0], values[1]]),
        ParameterKind::Vec3 | ParameterKind::Color { alpha: false, .. } => {
            Literal::Vec3([values[0], values[1], values[2]])
        }
        ParameterKind::Vec4 | ParameterKind::Color { alpha: true, .. } => {
            Literal::Vec4([values[0], values[1], values[2], values[3]])
        }
        ParameterKind::Int => Literal::Int(whole(values[0])?),
        ParameterKind::UInt => match whole(values[0])? {
            negative if negative < 0 => {
                return Err(attribute_error(name, format!("{negative} is not unsigned")));
            }
            value => Literal::UInt(value as u32),
        },
        ParameterKind::Bool => Literal::Bool(values[0] != 0.0),
        ParameterKind::Enum { options } => {
            let index = whole(values[0])?;
            if index < 0 || index as usize >= options.len() {
                return Err(attribute_error(
                    name,
                    format!("option {index} is out of range"),
                ));
            }
            Literal::Enum(index as u32)
        }
        ParameterKind::Texture { .. } => {
            return Err(attribute_error(name, "textures take no value".into()));
        }
    })
}

fn zero(kind: &ParameterKind) -> Literal {
    match kind {
        ParameterKind::Float => Literal::Float(0.0),
        ParameterKind::Vec2 => Literal::Vec2([0.0; 2]),
        ParameterKind::Vec3 | ParameterKind::Color { alpha: false, .. } => Literal::Vec3([0.0; 3]),
        ParameterKind::Vec4 | ParameterKind::Color { alpha: true, .. } => Literal::Vec4([0.0; 4]),
        ParameterKind::Int => Literal::Int(0),
        ParameterKind::UInt => Literal::UInt(0),
        ParameterKind::Bool => Literal::Bool(false),
        ParameterKind::Enum { .. } => Literal::Enum(0),
        ParameterKind::Texture { .. } => Literal::Texture,
    }
}

fn bounds(
    field: &Field,
    attribute: &str,
    kind: &ParameterKind,
    name: &str,
) -> Result<Option<Bounds>, SchemaError> {
    let Some(found) = field.attribute(attribute) else {
        return Ok(None);
    };
    if !kind.is_numeric() {
        return Err(attribute_error(
            name,
            format!("{attribute} needs a numeric parameter"),
        ));
    }
    let low = number(found, 0, name)?;
    let high = number(found, 1, name)?;
    if low > high {
        return Err(attribute_error(
            name,
            format!("{attribute} {low} is above {high}"),
        ));
    }
    Ok(Some(Bounds { low, high }))
}

fn feature(field: &Field, name: &str) -> Result<Option<u32>, SchemaError> {
    let Some(attribute) = field.attribute("Feature") else {
        return Ok(None);
    };
    let bit = number(attribute, 0, name)?;
    if !(0.0..32.0).contains(&bit) || bit.fract() != 0.0 {
        return Err(attribute_error(
            name,
            format!("feature bit {bit} is not a whole number below 32"),
        ));
    }
    Ok(Some(bit as u32))
}

fn hints(field: &Field, name: &str) -> Result<Vec<Hint>, SchemaError> {
    field
        .attributes
        .iter()
        .filter(|attribute| attribute.name == "Hint")
        .map(
            |attribute| match attribute.arguments.first().and_then(Argument::as_str) {
                Some("angle") => Ok(Hint::Angle),
                Some("logarithmic") => Ok(Hint::Logarithmic),
                Some("hidden") => Ok(Hint::Hidden),
                Some("advanced") => Ok(Hint::Advanced),
                other => Err(attribute_error(name, format!("unknown hint {other:?}"))),
            },
        )
        .collect()
}

fn role(field: &Field, kind: &ParameterKind, name: &str) -> Result<Option<Role>, SchemaError> {
    let Some(attribute) = field.attribute("Role") else {
        return Ok(None);
    };
    let named = attribute.arguments.first().and_then(Argument::as_str);
    let Some(role) = Role::ALL
        .into_iter()
        .find(|role| named == Some(role.name()))
    else {
        return Err(attribute_error(name, format!("unknown role {named:?}")));
    };
    match role.fits(kind) {
        true => Ok(Some(role)),
        false => Err(attribute_error(
            name,
            format!("role {role:?} does not fit a {kind:?} parameter"),
        )),
    }
}

fn presets(field: &Field, kind: &ParameterKind, name: &str) -> Result<Vec<Preset>, SchemaError> {
    let mut presets = Vec::new();
    for attribute in field
        .attributes
        .iter()
        .filter(|attribute| attribute.name == "Preset")
    {
        let Some((names, values)) = attribute.arguments.split_first() else {
            return Err(attribute_error(
                name,
                "Preset needs names and a value".into(),
            ));
        };
        let Some(names) = names.as_str() else {
            return Err(attribute_error(name, "Preset needs names first".into()));
        };
        let value = literal(kind, values, name)?;
        for preset in names.split('|') {
            if presets.iter().any(|known: &Preset| known.name == preset) {
                return Err(attribute_error(
                    name,
                    format!("preset {preset} is given twice"),
                ));
            }
            presets.push(Preset {
                name: preset.to_owned(),
                value,
            });
        }
    }
    Ok(presets)
}
