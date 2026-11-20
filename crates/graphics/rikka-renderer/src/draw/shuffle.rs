use std::sync::Arc;

use ash::vk;

use crate::draw::arena::Move;
use rikka_gpu::core::buffer::GpuBuffer;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::memory::Placement;
use rikka_gpu::core::runtime::Recycler;
use rikka_gpu::error::Result;

#[derive(Default)]
pub(crate) struct Shuffle {
    scratch: Option<GpuBuffer>,
    out: Vec<vk::BufferCopy>,
    back: Vec<vk::BufferCopy>,
}

impl Shuffle {
    pub(crate) fn trim(&mut self, recycler: &mut Recycler<'_>) {
        if let Some(scratch) = self.scratch.take() {
            recycler.retire.defer(recycler.value, Box::new(scratch));
        }
    }
    pub(crate) fn apply(
        &mut self,
        device: &Arc<RenderDevice>,
        ctx: &mut GpuFrameCtx<'_>,
        buffer: &GpuBuffer,
        moves: &[Move],
        stride: vk::DeviceSize,
    ) -> Result<bool> {
        self.out.clear();
        self.back.clear();
        let mut parked: vk::DeviceSize = 0;
        for moved in moves.iter().filter(|moved| moved.from != moved.to) {
            let unit = |at: u32| vk::DeviceSize::from(at) * stride;
            let size = unit(moved.count)
                .min(buffer.size().saturating_sub(unit(moved.from)))
                .min(buffer.size().saturating_sub(unit(moved.to)));
            if size == 0 {
                continue;
            }
            self.out.push(vk::BufferCopy {
                src_offset: unit(moved.from),
                dst_offset: parked,
                size,
            });
            self.back.push(vk::BufferCopy {
                src_offset: parked,
                dst_offset: unit(moved.to),
                size,
            });
            parked += size;
        }
        if parked == 0 {
            return Ok(false);
        }
        self.make_room(device, ctx, parked)?;
        let scratch = self.scratch.as_ref().expect("scratch buffer has room");
        let device = ctx.device.handle();
        let read = (
            vk::PipelineStageFlags2::COPY,
            vk::AccessFlags2::TRANSFER_READ,
        );
        let write = (
            vk::PipelineStageFlags2::COPY,
            vk::AccessFlags2::TRANSFER_WRITE,
        );
        let readers = (
            vk::PipelineStageFlags2::VERTEX_SHADER | vk::PipelineStageFlags2::COPY,
            vk::AccessFlags2::SHADER_STORAGE_READ
                | vk::AccessFlags2::TRANSFER_READ
                | vk::AccessFlags2::TRANSFER_WRITE,
        );
        ctx.begin_pass("rikka.instances.shuffle");
        unsafe {
            device.cmd_pipeline_barrier2(
                ctx.cmd,
                &vk::DependencyInfo::default().buffer_memory_barriers(&[
                    barrier(buffer, (readers.0, vk::AccessFlags2::TRANSFER_WRITE), read),
                    barrier(scratch, (read.0, vk::AccessFlags2::NONE), write),
                ]),
            );
            device.cmd_copy_buffer(ctx.cmd, buffer.handle(), scratch.handle(), &self.out);
            device.cmd_pipeline_barrier2(
                ctx.cmd,
                &vk::DependencyInfo::default().buffer_memory_barriers(&[
                    barrier(scratch, write, read),
                    barrier(buffer, (read.0, vk::AccessFlags2::NONE), write),
                ]),
            );
            device.cmd_copy_buffer(ctx.cmd, scratch.handle(), buffer.handle(), &self.back);
            device.cmd_pipeline_barrier2(
                ctx.cmd,
                &vk::DependencyInfo::default()
                    .buffer_memory_barriers(&[barrier(buffer, write, readers)]),
            );
        }
        ctx.end_pass();
        Ok(true)
    }

    fn make_room(
        &mut self,
        device: &Arc<RenderDevice>,
        ctx: &mut GpuFrameCtx<'_>,
        size: vk::DeviceSize,
    ) -> Result<()> {
        if self
            .scratch
            .as_ref()
            .is_none_or(|scratch| scratch.size() < size)
        {
            let grown = GpuBuffer::new(
                device.clone(),
                "rikka.instances.scratch",
                size.next_power_of_two(),
                vk::BufferUsageFlags::TRANSFER_SRC | vk::BufferUsageFlags::TRANSFER_DST,
                Placement::Device,
            )?;
            if let Some(old) = self.scratch.replace(grown) {
                ctx.retire.defer(ctx.frame_value, Box::new(old));
            }
        }
        Ok(())
    }
}

fn barrier(
    buffer: &GpuBuffer,
    (from_stage, from_access): (vk::PipelineStageFlags2, vk::AccessFlags2),
    (to_stage, to_access): (vk::PipelineStageFlags2, vk::AccessFlags2),
) -> vk::BufferMemoryBarrier2<'static> {
    vk::BufferMemoryBarrier2::default()
        .src_stage_mask(from_stage)
        .src_access_mask(from_access)
        .dst_stage_mask(to_stage)
        .dst_access_mask(to_access)
        .src_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .dst_queue_family_index(vk::QUEUE_FAMILY_IGNORED)
        .buffer(buffer.handle())
        .offset(0)
        .size(vk::WHOLE_SIZE)
}
