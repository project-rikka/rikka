use std::sync::Arc;

use ash::vk;
use rikka_spirv::Shader;

use crate::compute::context::Shared;
use crate::compute::lane::{Lane, Uses};
use crate::core::compute::create_compute_pipeline;
use crate::core::device::RenderDevice;
use crate::error::Result;

#[derive(Clone)]
pub struct Kernel {
    pub(crate) inner: Arc<KernelInner>,
}

pub(crate) struct KernelInner {
    shared: Arc<Shared>,
    pipeline: vk::Pipeline,
    shader: Shader,
    uses: Uses,
}

struct RetiredPipeline {
    device: Arc<RenderDevice>,
    pipeline: vk::Pipeline,
}

impl Kernel {
    pub(crate) fn new(shared: &Arc<Shared>, shader: Shader) -> Result<Kernel> {
        let mut kernels = shared.kernels.lock();
        if let Some(inner) = kernels.get(&shader.hash).and_then(std::sync::Weak::upgrade)
            && inner.shader.spirv == shader.spirv
        {
            return Ok(Kernel { inner });
        }
        let pipeline =
            create_compute_pipeline(&shared.device, shader.spirv, shared.table.pipeline_layout())?;
        shared
            .device
            .set_object_name(pipeline, "rikka.compute.kernel");
        let inner = Arc::new(KernelInner {
            shared: Arc::clone(shared),
            pipeline,
            shader,
            uses: Uses::default(),
        });
        kernels.retain(|_, kernel| kernel.strong_count() > 0);
        kernels.insert(shader.hash, Arc::downgrade(&inner));
        Ok(Kernel { inner })
    }

    pub(crate) fn pipeline(&self) -> vk::Pipeline {
        self.inner.pipeline
    }

    pub(crate) fn belongs_to(&self, shared: &Arc<Shared>) -> bool {
        Arc::ptr_eq(&self.inner.shared, shared)
    }

    pub(crate) fn used_until(&self, lane: Lane, value: u64) {
        self.inner.uses.mark(lane, value);
    }
}

impl Drop for KernelInner {
    fn drop(&mut self) {
        let last = self.uses.marks();
        self.shared.retire(
            last,
            Box::new(RetiredPipeline {
                device: self.shared.device.clone(),
                pipeline: self.pipeline,
            }),
        );
    }
}

impl Drop for RetiredPipeline {
    fn drop(&mut self) {
        unsafe { self.device.handle().destroy_pipeline(self.pipeline, None) };
    }
}

impl std::fmt::Debug for Kernel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Kernel").finish_non_exhaustive()
    }
}
