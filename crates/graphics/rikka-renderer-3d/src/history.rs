use std::hash::{Hash, Hasher};

use ash::vk;
use glam::Mat4;
use rustc_hash::FxHasher;

use crate::error::Result;
use crate::gpu::GpuInstance;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;
use rikka_gpu::core::runtime::Recycler;

#[derive(Default)]
pub(crate) struct ViewHistory {
    pub view_projection: Option<Mat4>,
    pub time: Option<f32>,
    pub environment_rotation: Option<f32>,
    pub frames: u64,
    positions: Option<GpuBuffer>,
    signature: Option<u64>,
    transforms: Vec<[f32; 16]>,
    instances: Option<u64>,
}

fn instance_key(instances: &[GpuInstance]) -> u64 {
    let mut hasher = FxHasher::default();
    for instance in instances {
        (
            instance.positions,
            instance.first_index,
            instance.index_count,
            instance.object_id,
        )
            .hash(&mut hasher);
    }
    hasher.finish()
}

impl ViewHistory {
    pub(crate) fn previous_transforms(
        &mut self,
        instances: &[GpuInstance],
        continuous: bool,
    ) -> &[[f32; 16]] {
        let known = continuous && self.instances == Some(instance_key(instances));
        if !known {
            self.remember_transforms(instances);
        }
        &self.transforms
    }

    pub(crate) fn remember_transforms(&mut self, instances: &[GpuInstance]) {
        self.transforms.clear();
        self.transforms
            .extend(instances.iter().map(|instance| instance.transform));
        self.instances = Some(instance_key(instances));
    }

    pub(crate) fn previous_positions(&self, signature: u64) -> Option<vk::DeviceAddress> {
        match (&self.positions, self.signature) {
            (Some(buffer), Some(stored)) if stored == signature => Some(buffer.device_address()),
            _ => None,
        }
    }

    pub(crate) fn reserve(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        bytes: u64,
    ) -> Result<Option<vk::Buffer>> {
        self.signature = None;
        if bytes == 0 {
            if let Some(buffer) = self.positions.take() {
                ctx.retire.defer(ctx.frame_value, Box::new(buffer));
            }
            return Ok(None);
        }
        let fits = self
            .positions
            .as_ref()
            .is_some_and(|buffer| buffer.size() >= bytes);
        if !fits {
            let replacement = GpuBuffer::addressable(
                ctx.device.clone(),
                "rikka.three.previous_positions",
                bytes.next_power_of_two(),
                vk::BufferUsageFlags::STORAGE_BUFFER | vk::BufferUsageFlags::TRANSFER_DST,
                Placement::Device,
            )?;
            if let Some(previous) = self.positions.replace(replacement) {
                ctx.retire.defer(ctx.frame_value, Box::new(previous));
            }
        }
        Ok(self.positions.as_ref().map(GpuBuffer::handle))
    }

    pub(crate) fn remember(&mut self, signature: u64) {
        if self.positions.is_some() {
            self.signature = Some(signature);
        }
    }

    pub(crate) fn release(self, recycler: &mut Recycler<'_>) {
        if let Some(buffer) = self.positions {
            recycler.retire.defer(recycler.value, Box::new(buffer));
        }
    }
}
