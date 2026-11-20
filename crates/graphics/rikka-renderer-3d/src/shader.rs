use std::borrow::Cow;
use std::sync::Arc;

use rikka_spirv::{Descriptor, ENTRY_POINT, Model, Reflection, Shader};

use crate::error::{Error, Result};
use crate::state::PUSH_SIZE;

#[derive(Clone, Debug)]
pub(crate) struct MaterialShader {
    inner: Arc<ShaderInner>,
}

#[derive(Debug)]
struct ShaderInner {
    spirv: Cow<'static, [u8]>,
    hash: u64,
    storage_images: bool,
}

const TABLE_SET: u32 = 0;

const ALLOWED_CAPABILITIES: &[u32] = &[
    0, 1, 11, 35, 43, 44, 46, 47, 49, 50, 51, 52, 56, 5301, 5302, 5307, 5308, 5309, 5347, 5379,
];

const RAY_QUERY_CAPABILITY: u32 = 4472;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tracing {
    Raster,
    RayQueries,
}

impl Tracing {
    fn allows(self, capability: u32) -> bool {
        ALLOWED_CAPABILITIES.contains(&capability)
            || (self == Self::RayQueries && capability == RAY_QUERY_CAPABILITY)
    }
}

impl MaterialShader {
    pub(crate) fn from_spirv(spirv: Vec<u8>, tracing: Tracing) -> Result<Self> {
        Ok(Self::reflected(spirv, tracing)?.0)
    }

    pub(crate) fn reflected(spirv: Vec<u8>, tracing: Tracing) -> Result<(Self, Reflection)> {
        let words = rikka_spirv::words(&spirv).map_err(refused)?;
        let reflection = rikka_spirv::reflect(&words, ENTRY_POINT).map_err(refused)?;
        check_stage(&reflection)?;
        check_table_bindings(&reflection, tracing)?;
        let hash = rikka_spirv::content_hash(&spirv);
        let shader = Self::build(Cow::Owned(spirv), hash, reflection.uses_storage_images());
        Ok((shader, reflection))
    }

    pub(crate) fn builtin(shader: Shader) -> Self {
        Self::build(
            Cow::Borrowed(shader.spirv),
            shader.hash,
            shader.uses_storage_images,
        )
    }

    fn build(spirv: Cow<'static, [u8]>, hash: u64, storage_images: bool) -> Self {
        Self {
            inner: Arc::new(ShaderInner {
                spirv,
                hash,
                storage_images,
            }),
        }
    }

    pub(crate) fn spirv(&self) -> &[u8] {
        &self.inner.spirv
    }

    pub(crate) fn hash(&self) -> u64 {
        self.inner.hash
    }

    pub(crate) fn uses_storage_images(&self) -> bool {
        self.inner.storage_images
    }
}

fn refused(error: rikka_spirv::Error) -> Error {
    Error::InvalidShader(format!("invalid SPIR-V: {error}"))
}

fn check_stage(reflection: &Reflection) -> Result<()> {
    match reflection.models.as_slice() {
        [Model::Vertex | Model::Fragment | Model::Compute] => Ok(()),
        [single] => Err(Error::InvalidShader(format!(
            "`{ENTRY_POINT}` is a {single:?} shader; materials need vertex, fragment and compute \
             stages"
        ))),
        _ => Err(Error::InvalidShader(format!(
            "shader has more than one `{ENTRY_POINT}` entry point"
        ))),
    }
}

fn check_table_bindings(reflection: &Reflection, tracing: Tracing) -> Result<()> {
    if let Some(capability) = reflection
        .capabilities
        .iter()
        .find(|capability| !tracing.allows(**capability))
    {
        return Err(Error::InvalidShader(format!(
            "shader declares SPIR-V capability {capability}, which the 3D renderer does not \
             enable"
        )));
    }
    if reflection.push_blocks > 1 {
        return Err(Error::InvalidShader(
            "shader declares more than one push constant block".into(),
        ));
    }
    if reflection.push_size > PUSH_SIZE {
        return Err(Error::InvalidShader(format!(
            "shader push constants use {} bytes, limit is {PUSH_SIZE}",
            reflection.push_size
        )));
    }
    for binding in &reflection.bindings {
        let expected = match (binding.set, binding.binding) {
            (TABLE_SET, 0) => Descriptor::SampledImage2d,
            (TABLE_SET, 1) => Descriptor::Image2d,
            (TABLE_SET, 2) => Descriptor::Sampler,
            (TABLE_SET, 3) => Descriptor::StorageImage2d,
            (TABLE_SET, 4) => Descriptor::Buffer,
            (set, binding) => {
                return Err(Error::InvalidShader(format!(
                    "shader uses set {set} binding {binding}, which is not in the rikka \
                     descriptor table"
                )));
            }
        };
        if binding.descriptor != expected {
            return Err(Error::InvalidShader(format!(
                "shader uses set {} binding {} as {:?}; the rikka table expects \
                 {expected:?}",
                binding.set, binding.binding, binding.descriptor
            )));
        }
    }
    if reflection.models == [Model::Vertex] && !reflection.inputs.is_empty() {
        return Err(Error::InvalidShader(
            "material vertex shaders fetch vertices by buffer address and must not declare \
             vertex inputs"
                .into(),
        ));
    }
    Ok(())
}
