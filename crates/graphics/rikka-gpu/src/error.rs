use std::error::Error as StdError;

use ash::vk;

use crate::texture::TextureId;

pub type Cause = Box<dyn StdError + Send + Sync>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to load the Vulkan loader")]
    Loading(#[source] Cause),

    #[error("failed to load Vulkan driver {}", path.display())]
    DriverLoading {
        path: std::path::PathBuf,
        #[source]
        source: Cause,
    },

    #[error("Vulkan call failed: {0}")]
    Driver(Cause),

    #[error("no suitable GPU found: {0}")]
    NoSuitableDevice(String),

    #[error("GPU memory allocation failed")]
    Allocation(#[source] Cause),

    #[error("unsupported by this device: {0}")]
    Unsupported(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("{0} does not exist")]
    Unknown(Resource),

    #[error("renderer was built without {0}")]
    NotEnabled(Capability),

    #[error("{0} is full")]
    Full(Store),

    #[error("window surface stayed stale")]
    Unsettled,

    #[error("scanout waiter failed")]
    ScanoutWaiter(#[source] Cause),

    #[error("invalid SPIR-V shader: {0}")]
    InvalidShader(String),

    #[error("failed to save pipeline cache to {}", path.display())]
    PipelineCache {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Resource {
    #[error("texture {0:?}")]
    Texture(TextureId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Store {
    #[error("glyph atlas of {pages} pages")]
    GlyphAtlas { pages: usize },
    #[error("glyph curve store of {words} words")]
    GlyphCurves { words: u32 },
    #[error("{what} table of {slots} slots")]
    Table { what: &'static str, slots: u32 },
    #[error("indirect draw of at most {draws} draws")]
    IndirectDraws { draws: u32 },
    #[error("per-frame deformed vertex buffer")]
    DeformedVertices,
    #[error("{0} arena")]
    Arena(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Capability {
    #[error("3D: build it with GpuBuilder::with_3d()")]
    ThreeD,
    #[error("compute: build it with GpuBuilder::compute_support() on a capable device")]
    Compute,
    #[error("ray tracing: build it with GpuBuilder::ray_tracing() on a capable device")]
    RayTracing,
}

impl From<ash::LoadingError> for Error {
    fn from(source: ash::LoadingError) -> Self {
        Self::Loading(Box::new(source))
    }
}

impl From<vk::Result> for Error {
    fn from(source: vk::Result) -> Self {
        Self::Driver(Box::new(source))
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
