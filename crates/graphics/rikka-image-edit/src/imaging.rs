use std::sync::{Arc, OnceLock};

use rikka_gpu::{Compute, Gpu, ImageFormat, Kernel};
use rikka_sync::Mutex;

use crate::color::{Profile, Space};
use crate::error::Result;
use crate::shader::Shader;

const MAX_CACHED_SPACES: usize = 16;

static SHARED: OnceLock<Imaging> = OnceLock::new();

#[derive(Clone)]
pub struct Imaging {
    inner: Arc<Engine>,
}

struct Engine {
    compute: Compute,
    kernels: Mutex<Vec<Option<Kernel>>>,
    spaces: Mutex<Vec<Arc<Space>>>,
    formats: Formats,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Formats {
    pub(crate) deep: ImageFormat,
    pub(crate) shallow_mask: ImageFormat,
    pub(crate) deep_mask: ImageFormat,
}

impl Imaging {
    #[must_use]
    pub fn new(compute: Compute) -> Imaging {
        let pick = |preferred: ImageFormat, fallback: ImageFormat| match compute.supports(preferred)
        {
            true => preferred,
            false => fallback,
        };
        let formats = Formats {
            deep: pick(ImageFormat::Rgba16, ImageFormat::Rgba32Float),
            shallow_mask: pick(ImageFormat::R8, ImageFormat::R32Float),
            deep_mask: pick(ImageFormat::R16, ImageFormat::R32Float),
        };
        Imaging {
            inner: Arc::new(Engine {
                compute,
                kernels: Mutex::new(vec![None; Shader::COUNT]),
                spaces: Mutex::new(Vec::new()),
                formats,
            }),
        }
    }

    #[must_use]
    pub fn shared(compute: &Compute) -> Imaging {
        SHARED
            .get_or_init(|| Imaging::new(compute.background()))
            .clone()
    }

    pub fn headless() -> Result<Imaging> {
        let gpu = Gpu::builder()
            .app_name("rikka-image-edit")
            .compute_support()
            .build()?;
        Ok(Imaging::new(gpu.compute()?))
    }

    #[must_use]
    pub fn compute(&self) -> &Compute {
        &self.inner.compute
    }

    pub(crate) fn formats(&self) -> Formats {
        self.inner.formats
    }

    pub(crate) fn kernel(&self, shader: Shader) -> Result<Kernel> {
        let mut kernels = self.inner.kernels.lock();
        if let Some(kernel) = &kernels[shader.index()] {
            return Ok(kernel.clone());
        }
        let kernel = self.inner.compute.kernel(shader.compiled())?;
        kernels[shader.index()] = Some(kernel.clone());
        Ok(kernel)
    }

    pub(crate) fn space(&self, profile: Option<&Profile>) -> Arc<Space> {
        let mut spaces = self.inner.spaces.lock();
        if let Some(cached) = spaces.iter().find(|space| space.profile() == profile) {
            return Arc::clone(cached);
        }
        let space = Arc::new(Space::new(profile));
        if spaces.len() == MAX_CACHED_SPACES {
            spaces.remove(0);
        }
        spaces.push(Arc::clone(&space));
        space
    }
}

impl std::fmt::Debug for Imaging {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Imaging")
            .field("compute", &self.inner.compute)
            .finish_non_exhaustive()
    }
}
