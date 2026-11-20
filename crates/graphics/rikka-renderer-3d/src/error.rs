use ash::vk;
use rikka_gpu::error::{Capability, Store};

use crate::effect::EffectId;
use crate::light::LightProfileId;
use crate::material::MaterialId;
use crate::mesh::MeshId;
use crate::overlay::OverlayBufferId;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Gpu(#[from] rikka_gpu::Error),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("invalid SPIR-V shader: {0}")]
    InvalidShader(String),

    #[error("{0} does not exist")]
    Unknown(Resource),

    #[error(transparent)]
    Slang(#[from] rikka_slang::Error),

    #[error("invalid material parameters: {0}")]
    Schema(#[from] rikka_slang::SchemaError),

    #[error(transparent)]
    Parameter(#[from] ParameterError),
}

impl Error {
    pub(crate) fn unsupported(message: String) -> Self {
        Self::Gpu(rikka_gpu::Error::Unsupported(message))
    }

    pub(crate) fn full(store: Store) -> Self {
        Self::Gpu(rikka_gpu::Error::Full(store))
    }

    pub(crate) fn not_enabled(capability: Capability) -> Self {
        Self::Gpu(rikka_gpu::Error::NotEnabled(capability))
    }
}

impl From<vk::Result> for Error {
    fn from(source: vk::Result) -> Self {
        Self::Gpu(source.into())
    }
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ParameterError {
    #[error("{owner} has no parameter {name}")]
    Unknown { owner: String, name: String },
    #[error("parameter {name} is {expected:?}, not {given}")]
    Mismatch {
        name: String,
        expected: rikka_slang::ParameterKind,
        given: &'static str,
    },
    #[error("parameter {name} = {value} is outside {low}..={high}")]
    OutOfRange {
        name: String,
        value: f32,
        low: f32,
        high: f32,
    },
    #[error("parameter {name} has no option {option}")]
    UnknownOption { name: String, option: String },
    #[error("{owner} has no preset {name}")]
    UnknownPreset { owner: String, name: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Resource {
    #[error("mesh {0:?}")]
    Mesh(MeshId),
    #[error("material {0:?}")]
    Material(MaterialId),
    #[error("effect {0:?}")]
    Effect(EffectId),
    #[error("light profile {0:?}")]
    LightProfile(LightProfileId),
    #[error("overlay buffer {0:?}")]
    OverlayBuffer(OverlayBufferId),
}
