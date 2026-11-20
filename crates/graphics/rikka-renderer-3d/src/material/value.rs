use std::hash::{Hash, Hasher};

use glam::{Vec2, Vec3, Vec4};
use rikka_slang::{Literal, ParameterKind, ParameterSchema, ParameterSpec};

use crate::error::{ParameterError, Result};
use crate::gpu::NONE_INDEX;
use rikka_gpu::core::sampler::Filtering;
use rikka_gpu::texture::TextureId;

pub(crate) const BLOCK_ALIGNMENT: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureBinding {
    pub texture: TextureId,
    pub filtering: Filtering,
}

impl TextureBinding {
    pub fn new(texture: TextureId, filtering: Filtering) -> Self {
        Self { texture, filtering }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Float(f32),
    Vec2(Vec2),
    Vec3(Vec3),
    Vec4(Vec4),
    Int(i32),
    UInt(u32),
    Bool(bool),
    Enum(String),
    Texture(Option<TextureBinding>),
}

impl Value {
    pub(crate) fn kind_name(&self) -> &'static str {
        match self {
            Self::Float(_) => "float",
            Self::Vec2(_) => "vec2",
            Self::Vec3(_) => "vec3",
            Self::Vec4(_) => "vec4",
            Self::Int(_) => "int",
            Self::UInt(_) => "uint",
            Self::Bool(_) => "bool",
            Self::Enum(_) => "enum",
            Self::Texture(_) => "texture",
        }
    }

    pub(crate) fn from_literal(spec: &ParameterSpec, literal: Literal) -> Self {
        match (literal, &spec.kind) {
            (Literal::Float(value), _) => Self::Float(value),
            (Literal::Vec2(value), _) => Self::Vec2(Vec2::from_array(value)),
            (Literal::Vec3(value), _) => Self::Vec3(Vec3::from_array(value)),
            (Literal::Vec4(value), _) => Self::Vec4(Vec4::from_array(value)),
            (Literal::Int(value), _) => Self::Int(value),
            (Literal::UInt(value), _) => Self::UInt(value),
            (Literal::Bool(value), _) => Self::Bool(value),
            (Literal::Enum(index), ParameterKind::Enum { options }) => {
                Self::Enum(options[index as usize].clone())
            }
            (Literal::Enum(index), _) => Self::Int(index as i32),
            (Literal::Texture, _) => Self::Texture(None),
        }
    }

    pub(crate) fn check(&self, spec: &ParameterSpec) -> Result<(), ParameterError> {
        let fits = matches!(
            (self, &spec.kind),
            (Self::Float(_), ParameterKind::Float)
                | (Self::Vec2(_), ParameterKind::Vec2)
                | (
                    Self::Vec3(_),
                    ParameterKind::Vec3 | ParameterKind::Color { alpha: false, .. }
                )
                | (
                    Self::Vec4(_),
                    ParameterKind::Vec4 | ParameterKind::Color { alpha: true, .. }
                )
                | (Self::Int(_), ParameterKind::Int)
                | (Self::UInt(_), ParameterKind::UInt)
                | (Self::Bool(_), ParameterKind::Bool)
                | (Self::Enum(_), ParameterKind::Enum { .. })
                | (Self::Texture(_), ParameterKind::Texture { .. })
        );
        if !fits {
            return Err(ParameterError::Mismatch {
                name: spec.name.clone(),
                expected: spec.kind.clone(),
                given: self.kind_name(),
            });
        }
        if let (Self::Enum(option), ParameterKind::Enum { options }) = (self, &spec.kind)
            && !options.contains(option)
        {
            return Err(ParameterError::UnknownOption {
                name: spec.name.clone(),
                option: option.clone(),
            });
        }
        if let Some(range) = spec.range
            && let Some(outside) = self
                .components()
                .iter()
                .copied()
                .find(|value| !(range.low..=range.high).contains(value))
        {
            return Err(ParameterError::OutOfRange {
                name: spec.name.clone(),
                value: outside,
                low: range.low,
                high: range.high,
            });
        }
        Ok(())
    }

    fn components(&self) -> smallvec::SmallVec<[f32; 4]> {
        match self {
            Self::Float(value) => smallvec::smallvec![*value],
            Self::Vec2(value) => value.to_array().into_iter().collect(),
            Self::Vec3(value) => value.to_array().into_iter().collect(),
            Self::Vec4(value) => value.to_array().into_iter().collect(),
            Self::Int(value) => smallvec::smallvec![*value as f32],
            Self::UInt(value) => smallvec::smallvec![*value as f32],
            Self::Bool(_) | Self::Enum(_) | Self::Texture(_) => smallvec::SmallVec::new(),
        }
    }

    pub(crate) fn hash_into(&self, state: &mut impl Hasher) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Enum(option) => option.hash(state),
            Self::Bool(value) => value.hash(state),
            Self::Int(value) => value.hash(state),
            Self::UInt(value) => value.hash(state),
            Self::Texture(binding) => binding.hash(state),
            other => other
                .components()
                .iter()
                .for_each(|value| value.to_bits().hash(state)),
        }
    }

    pub(crate) fn texture(&self) -> Option<TextureBinding> {
        match self {
            Self::Texture(binding) => *binding,
            _ => None,
        }
    }
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Self::Float(value)
    }
}

impl From<Vec2> for Value {
    fn from(value: Vec2) -> Self {
        Self::Vec2(value)
    }
}

impl From<Vec3> for Value {
    fn from(value: Vec3) -> Self {
        Self::Vec3(value)
    }
}

impl From<Vec4> for Value {
    fn from(value: Vec4) -> Self {
        Self::Vec4(value)
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Self {
        Self::Int(value)
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Self::UInt(value)
    }
}

impl From<[f32; 2]> for Value {
    fn from(value: [f32; 2]) -> Self {
        Self::Vec2(Vec2::from_array(value))
    }
}

impl From<[f32; 3]> for Value {
    fn from(value: [f32; 3]) -> Self {
        Self::Vec3(Vec3::from_array(value))
    }
}

impl From<[f32; 4]> for Value {
    fn from(value: [f32; 4]) -> Self {
        Self::Vec4(Vec4::from_array(value))
    }
}

impl From<TextureId> for Value {
    fn from(value: TextureId) -> Self {
        Self::Texture(Some(TextureBinding::new(value, Filtering::default())))
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<TextureBinding> for Value {
    fn from(value: TextureBinding) -> Self {
        Self::Texture(Some(value))
    }
}

impl From<Option<TextureBinding>> for Value {
    fn from(value: Option<TextureBinding>) -> Self {
        Self::Texture(value)
    }
}

pub(crate) struct Encoded {
    pub bytes: Vec<u8>,
    pub textures: Vec<u32>,
}

pub(crate) fn encode(
    schema: &ParameterSchema,
    values: &[Value],
    texture_slot: &impl Fn(TextureId) -> Option<u32>,
) -> Result<Encoded> {
    let size = schema
        .size()
        .next_multiple_of(BLOCK_ALIGNMENT)
        .max(BLOCK_ALIGNMENT);
    let mut bytes = vec![0u8; size];
    let mut textures = Vec::new();
    for (spec, value) in schema.parameters().iter().zip(values) {
        textures.extend(write_value(spec, value, &mut bytes, texture_slot)?);
    }
    Ok(Encoded { bytes, textures })
}

fn put(block: &mut [u8], at: usize, word: u32) {
    block[at..at + 4].copy_from_slice(&word.to_le_bytes());
}

pub(crate) fn write_defaults(schema: &ParameterSchema, block: &mut [u8]) {
    block.fill(0);
    for spec in schema.parameters() {
        let at = spec.offset;
        match spec.default {
            Literal::Float(value) => put(block, at, value.to_bits()),
            Literal::Vec2(values) => values
                .iter()
                .enumerate()
                .for_each(|(index, value)| put(block, at + index * 4, value.to_bits())),
            Literal::Vec3(values) => values
                .iter()
                .enumerate()
                .for_each(|(index, value)| put(block, at + index * 4, value.to_bits())),
            Literal::Vec4(values) => values
                .iter()
                .enumerate()
                .for_each(|(index, value)| put(block, at + index * 4, value.to_bits())),
            Literal::Int(value) => put(block, at, value as u32),
            Literal::UInt(value) | Literal::Enum(value) => put(block, at, value),
            Literal::Bool(value) => put(block, at, u32::from(value)),
            Literal::Texture => {
                put(block, at, NONE_INDEX);
                if let ParameterKind::Texture { sampler: true, .. } = spec.kind {
                    put(block, at + 4, 0);
                }
            }
        }
    }
}

pub(crate) fn write_value(
    spec: &ParameterSpec,
    value: &Value,
    block: &mut [u8],
    texture_slot: &impl Fn(TextureId) -> Option<u32>,
) -> Result<Option<u32>> {
    let at = spec.offset;
    match value {
        Value::Bool(value) => put(block, at, u32::from(*value)),
        Value::Int(value) => put(block, at, *value as u32),
        Value::UInt(value) => put(block, at, *value),
        Value::Enum(option) => {
            let ParameterKind::Enum { options } = &spec.kind else {
                unreachable!("values are checked against their parameter");
            };
            let index = options
                .iter()
                .position(|known| known == option)
                .expect("enum values are checked against their options");
            put(block, at, index as u32);
        }
        Value::Texture(binding) => {
            let (image, sampler) = match binding {
                Some(binding) => (
                    texture_slot(binding.texture).ok_or(rikka_gpu::Error::Unknown(
                        rikka_gpu::Resource::Texture(binding.texture),
                    ))?,
                    binding.filtering.slot(),
                ),
                None => (NONE_INDEX, 0),
            };
            put(block, at, image);
            if let ParameterKind::Texture { sampler: true, .. } = spec.kind {
                put(block, at + 4, sampler);
            }
            return Ok(binding.map(|_| image));
        }
        Value::Float(value) => put(block, at, value.to_bits()),
        Value::Vec2(value) => write_floats(block, at, &value.to_array()),
        Value::Vec3(value) => write_floats(block, at, &value.to_array()),
        Value::Vec4(value) => write_floats(block, at, &value.to_array()),
    }
    Ok(None)
}

fn write_floats(block: &mut [u8], at: usize, values: &[f32]) {
    for (index, value) in values.iter().enumerate() {
        put(block, at + index * 4, value.to_bits());
    }
}
