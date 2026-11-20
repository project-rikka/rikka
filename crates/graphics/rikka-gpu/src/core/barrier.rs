use ash::vk;

use crate::core::device::RenderDevice;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryAccess {
    pub stage: vk::PipelineStageFlags2,
    pub access: vk::AccessFlags2,
}

impl MemoryAccess {
    pub const fn new(stage: vk::PipelineStageFlags2, access: vk::AccessFlags2) -> Self {
        Self { stage, access }
    }
}

pub fn record_memory_barrier(
    device: &RenderDevice,
    cmd: vk::CommandBuffer,
    from: MemoryAccess,
    to: MemoryAccess,
) {
    let barriers = [vk::MemoryBarrier2::default()
        .src_stage_mask(from.stage)
        .src_access_mask(from.access)
        .dst_stage_mask(to.stage)
        .dst_access_mask(to.access)];
    let dependency = vk::DependencyInfo::default().memory_barriers(&barriers);
    unsafe { device.handle().cmd_pipeline_barrier2(cmd, &dependency) };
}
