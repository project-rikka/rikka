use ash::vk;

use crate::core::device::RenderDevice;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageAccess {
    pub stage: vk::PipelineStageFlags2,
    pub access: vk::AccessFlags2,
    pub layout: vk::ImageLayout,
}

pub(super) const WRITE_ACCESS: vk::AccessFlags2 = vk::AccessFlags2::from_raw(
    vk::AccessFlags2::COLOR_ATTACHMENT_WRITE.as_raw()
        | vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE.as_raw()
        | vk::AccessFlags2::SHADER_STORAGE_WRITE.as_raw()
        | vk::AccessFlags2::TRANSFER_WRITE.as_raw(),
);

const TABLE_STAGES: vk::PipelineStageFlags2 = vk::PipelineStageFlags2::from_raw(
    vk::PipelineStageFlags2::VERTEX_SHADER.as_raw()
        | vk::PipelineStageFlags2::FRAGMENT_SHADER.as_raw()
        | vk::PipelineStageFlags2::COMPUTE_SHADER.as_raw(),
);

impl ImageAccess {
    pub const UNDEFINED: Self = Self {
        stage: vk::PipelineStageFlags2::NONE,
        access: vk::AccessFlags2::NONE,
        layout: vk::ImageLayout::UNDEFINED,
    };
    pub const COLOR_ATTACHMENT: Self = Self {
        stage: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        access: vk::AccessFlags2::from_raw(
            vk::AccessFlags2::COLOR_ATTACHMENT_READ.as_raw()
                | vk::AccessFlags2::COLOR_ATTACHMENT_WRITE.as_raw(),
        ),
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    };
    pub const FRAGMENT_SAMPLED: Self = Self {
        stage: vk::PipelineStageFlags2::FRAGMENT_SHADER,
        access: vk::AccessFlags2::SHADER_SAMPLED_READ,
        layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    };
    pub(crate) const TABLE_SAMPLED: Self = Self {
        stage: TABLE_STAGES,
        access: vk::AccessFlags2::SHADER_SAMPLED_READ,
        layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    };
    pub const TRANSFER_SRC: Self = Self {
        stage: vk::PipelineStageFlags2::TRANSFER,
        access: vk::AccessFlags2::TRANSFER_READ,
        layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
    };
    pub const TRANSFER_DST: Self = Self {
        stage: vk::PipelineStageFlags2::TRANSFER,
        access: vk::AccessFlags2::TRANSFER_WRITE,
        layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    };
    pub const EXTERNAL: Self = Self {
        stage: vk::PipelineStageFlags2::NONE,
        access: vk::AccessFlags2::NONE,
        layout: vk::ImageLayout::GENERAL,
    };
    pub const PRESENT: Self = Self {
        stage: vk::PipelineStageFlags2::NONE,
        access: vk::AccessFlags2::NONE,
        layout: vk::ImageLayout::PRESENT_SRC_KHR,
    };
    pub(crate) const FOREIGN_READY: Self = Self {
        stage: vk::PipelineStageFlags2::FRAGMENT_SHADER,
        access: vk::AccessFlags2::NONE,
        layout: vk::ImageLayout::GENERAL,
    };
    pub const COMPUTE_STORAGE: Self = Self {
        stage: vk::PipelineStageFlags2::COMPUTE_SHADER,
        access: vk::AccessFlags2::from_raw(
            vk::AccessFlags2::SHADER_STORAGE_READ.as_raw()
                | vk::AccessFlags2::SHADER_STORAGE_WRITE.as_raw(),
        ),
        layout: vk::ImageLayout::GENERAL,
    };

    pub fn after_waiting(mut self, stage: vk::PipelineStageFlags2) -> Self {
        self.stage |= stage;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueueTransfer {
    pub src: u32,
    pub dst: u32,
}

impl QueueTransfer {
    pub const NONE: Self = Self {
        src: vk::QUEUE_FAMILY_IGNORED,
        dst: vk::QUEUE_FAMILY_IGNORED,
    };

    pub const fn acquire(family: u32) -> Self {
        Self {
            src: vk::QUEUE_FAMILY_FOREIGN_EXT,
            dst: family,
        }
    }

    pub const fn release(family: u32) -> Self {
        Self {
            src: family,
            dst: vk::QUEUE_FAMILY_FOREIGN_EXT,
        }
    }

    pub const fn acquire_external(family: u32) -> Self {
        Self {
            src: vk::QUEUE_FAMILY_EXTERNAL,
            dst: family,
        }
    }

    pub const fn release_external(family: u32) -> Self {
        Self {
            src: family,
            dst: vk::QUEUE_FAMILY_EXTERNAL,
        }
    }
}

pub fn color_subresource_range() -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    }
}

pub fn color_subresource_layers() -> vk::ImageSubresourceLayers {
    vk::ImageSubresourceLayers {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        mip_level: 0,
        base_array_layer: 0,
        layer_count: 1,
    }
}

pub fn record_barriers(
    device: &RenderDevice,
    cmd: vk::CommandBuffer,
    barriers: &[vk::ImageMemoryBarrier2<'_>],
) {
    if barriers.is_empty() {
        return;
    }
    let dependency = vk::DependencyInfo::default().image_memory_barriers(barriers);
    unsafe { device.handle().cmd_pipeline_barrier2(cmd, &dependency) };
}

pub fn image_barrier(
    image: vk::Image,
    current: ImageAccess,
    next: ImageAccess,
    queues: QueueTransfer,
    range: vk::ImageSubresourceRange,
) -> vk::ImageMemoryBarrier2<'static> {
    vk::ImageMemoryBarrier2::default()
        .src_stage_mask(current.stage)
        .src_access_mask(current.access)
        .dst_stage_mask(next.stage)
        .dst_access_mask(next.access)
        .old_layout(current.layout)
        .new_layout(next.layout)
        .src_queue_family_index(queues.src)
        .dst_queue_family_index(queues.dst)
        .image(image)
        .subresource_range(range)
}
