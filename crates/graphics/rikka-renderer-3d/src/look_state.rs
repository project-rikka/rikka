use std::sync::Arc;

use ash::vk;

use crate::error::Result;
use crate::fluid::FluidState;
use crate::graph::ImageStore;
use crate::look::gpu::GpuEnvironment;
use crate::post::PostState;
use crate::shader::MaterialShader;
use crate::sky::SkyState;
use crate::space::SpaceState;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::memory::Placement;

pub(crate) struct LookState {
    pub images: ImageStore,
    pub environment: GpuBuffer,
    pub surface_vert: MaterialShader,
    pub effect_layers: MaterialShader,
    pub sky: SkyState,
    pub space: SpaceState,
    pub post: PostState,
    pub fluid: FluidState,
}

impl LookState {
    pub(crate) fn new(device: &Arc<RenderDevice>) -> Result<Self> {
        Ok(Self {
            images: ImageStore::default(),
            environment: GpuBuffer::addressable(
                device.clone(),
                "rikka.three.environment",
                size_of::<GpuEnvironment>() as u64,
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                Placement::Device,
            )?,
            surface_vert: MaterialShader::builtin(shaders_3d::SURFACE_VERT),
            effect_layers: MaterialShader::builtin(shaders_3d::EFFECT_LAYERS_FRAG),
            sky: SkyState::new(),
            space: SpaceState::new(),
            post: PostState::new(),
            fluid: FluidState::new(),
        })
    }
}
