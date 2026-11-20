use ash::vk;

use crate::draw::Frame;
use crate::error::Result;
use crate::graph::{Buffer, BufferUse, FrameGraph};
use crate::look::Look;
use crate::record::Record;
use crate::vertex::GpuPosition;
use crate::viewport::Viewport;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

#[derive(Clone, Copy)]
pub(super) struct HistoryCopy {
    source: vk::Buffer,
    target: vk::Buffer,
    bytes: u64,
}

impl Record<'_> {
    pub(super) fn reserve_history(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        viewport: &mut Viewport,
        looked: bool,
        blurred: bool,
        deform_vertices: u32,
    ) -> Result<Option<HistoryCopy>> {
        let temporal = viewport.temporal.is_some();
        if !looked && !temporal {
            return Ok(None);
        }
        let bytes = match blurred || temporal {
            true => u64::from(deform_vertices) * size_of::<GpuPosition>() as u64,
            false => 0,
        };
        let target = viewport.history.reserve(ctx, bytes)?;
        Ok(target.map(|target| HistoryCopy {
            source: self.mesh.frames.deform_positions(),
            target,
            bytes,
        }))
    }

    pub(super) fn declare_history(
        &mut self,
        graph: &mut FrameGraph<'_>,
        copy: Option<HistoryCopy>,
    ) {
        let Some(copy) = copy else {
            return;
        };
        graph
            .compute("rikka.three.history")
            .buffer(
                Buffer::Deform,
                BufferUse::new(
                    vk::PipelineStageFlags2::TRANSFER,
                    vk::AccessFlags2::TRANSFER_READ,
                ),
            )
            .buffer(
                Buffer::History,
                BufferUse::new(
                    vk::PipelineStageFlags2::TRANSFER,
                    vk::AccessFlags2::TRANSFER_WRITE,
                ),
            )
            .record(move |_, ctx, _| {
                let region = vk::BufferCopy::default().size(copy.bytes);
                unsafe {
                    ctx.device.handle().cmd_copy_buffer(
                        ctx.cmd,
                        copy.source,
                        copy.target,
                        &[region],
                    );
                }
                Ok(())
            });
    }

    pub(super) fn finish_history(
        &mut self,
        viewport: &mut Viewport,
        frame: &Frame,
        look: Option<&Look>,
        view_projection: glam::Mat4,
        signature: u64,
    ) {
        if look.is_some() || viewport.temporal.is_some() {
            viewport.history.remember(signature);
            viewport.history.view_projection = Some(view_projection);
            viewport.history.environment_rotation = look
                .and_then(|look| look.environment.image())
                .map(|image| image.rotation);
        }
        viewport.history.time = Some(frame.time);
        viewport.history.frames += 1;
        viewport.drawn = true;
    }
}
