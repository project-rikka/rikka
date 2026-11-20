use rikka_slang::{ParameterKind, ParameterSchema, Role};

use crate::material::{MaterialDesc, Value};

pub(crate) const ALL_FEATURES: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Routing {
    pub features: u32,
    pub subsurface: bool,
    pub transmission: bool,
}

impl Default for Routing {
    fn default() -> Self {
        Self {
            features: ALL_FEATURES,
            subsurface: false,
            transmission: false,
        }
    }
}

pub(crate) fn routing(schema: &ParameterSchema, values: &[Value]) -> Routing {
    let mut declared = false;
    let mut features = 0u32;
    for (spec, value) in schema.parameters().iter().zip(values) {
        let Some(bit) = spec.feature else {
            continue;
        };
        declared = true;
        if active(&spec.kind, value) {
            features |= 1 << bit;
        }
    }
    let routed = |role: Role| {
        schema.parameters().iter().zip(values).any(|(spec, value)| {
            spec.role == Some(role) && matches!(value, Value::Float(weight) if *weight > 0.0)
        })
    };
    Routing {
        features: match declared {
            true => features,
            false => ALL_FEATURES,
        },
        subsurface: routed(Role::Subsurface),
        transmission: routed(Role::Transmission),
    }
}

pub(crate) fn of(desc: &MaterialDesc) -> Routing {
    routing(&desc.program.schema, &desc.values)
}

fn active(kind: &ParameterKind, value: &Value) -> bool {
    match value {
        Value::Float(value) => *value != 0.0,
        Value::Vec2(value) => *value != glam::Vec2::ZERO,
        Value::Vec3(value) => *value != glam::Vec3::ZERO,
        Value::Vec4(value) => *value != glam::Vec4::ZERO,
        Value::Int(value) => *value != 0,
        Value::UInt(value) => *value != 0,
        Value::Bool(value) => *value,
        Value::Enum(name) => match kind {
            ParameterKind::Enum { options } => options.first() != Some(name),
            _ => true,
        },
        Value::Texture(binding) => binding.is_some(),
    }
}
