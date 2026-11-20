use ash::vk;
use rikka_colorimetry::HdrSpace;
use rikka_types::{Rect, Transform};

use crate::fx::frame_target::FrameTarget;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::ImageAccess;
use rikka_gpu::core::pipeline::Blend;
use rikka_gpu::error::Result;

use super::effect_output::Capture;
use super::pass_input::{PassInput, PassOutput};
use super::record::{Composite, Recorded, mapped};
use super::{COMPOSITE_SHADER, FxExecutor, OUTPUT_PQ_SHADER};

const SIGNAL_SCALE: usize = 6;

impl FxExecutor {
    pub(crate) fn composite(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        input: PassInput,
        output: PassOutput,
        transform: Transform,
        scissors: &[Rect],
    ) -> Result<()> {
        let result = self.record_pass(
            ctx,
            Recorded {
                label: "rikka.fx.composite",
                shader: &COMPOSITE_SHADER,
                params: mapped(transform),
                input,
                src_scene: None,
                output,
                load_op: vk::AttachmentLoadOp::LOAD,
                composite: Composite::Over(Blend::Opaque),
                scissors,
            },
        );
        self.unsettled(result)
    }

    pub(crate) fn encode_pq(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        input: PassInput,
        output: PassOutput,
        (transform, space): (Transform, HdrSpace),
        scissors: &[Rect],
    ) -> Result<()> {
        let mut params = mapped(transform);
        params[SIGNAL_SCALE] = space.signal_scale() as f32;
        let result = self.record_pass(
            ctx,
            Recorded {
                label: "rikka.fx.output_pq",
                shader: &OUTPUT_PQ_SHADER,
                params,
                input,
                src_scene: None,
                output,
                load_op: vk::AttachmentLoadOp::LOAD,
                composite: Composite::Over(Blend::Opaque),
                scissors,
            },
        );
        self.unsettled(result)
    }

    pub(crate) fn capture(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        frame_target: &FrameTarget,
        region: Rect,
    ) -> Result<Capture> {
        let result = self.take_in(ctx, frame_target, region);
        self.unsettled(result)
    }

    fn take_in(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        frame_target: &FrameTarget,
        region: Rect,
    ) -> Result<Capture> {
        let source = self.read_from(ctx, frame_target)?;
        let texels = frame_target.texels(region);
        let within = Rect::new(0.0, 0.0, texels.width(), texels.height());
        let index = self.pool.acquire(
            ctx,
            within.width() as u32,
            within.height() as u32,
            frame_target.format,
        )?;
        let barrier = self.pool.transition(index, ImageAccess::COLOR_ATTACHMENT);
        self.defer_barrier(barrier);
        let recorded = self.record_pass(
            ctx,
            Recorded {
                label: "rikka.fx.capture",
                shader: &COMPOSITE_SHADER,
                params: mapped(Transform::Normal),
                input: PassInput {
                    region: texels,
                    ..source
                },
                src_scene: None,
                output: PassOutput {
                    view: self.pool.image(index).view(),
                    format: frame_target.format,
                    region: within,
                },
                load_op: vk::AttachmentLoadOp::DONT_CARE,
                composite: Composite::Over(Blend::Opaque),
                scissors: &[within],
            },
        );
        self.recycle(frame_target);
        if let Err(error) = recorded {
            self.pool.release(index);
            return Err(error);
        }
        self.readable(index);
        Ok(Capture {
            pool_index: index,
            texels,
        })
    }

    pub(crate) fn mix_back(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        frame_target: &FrameTarget,
        capture: &Capture,
        mix: f32,
        scissors: &[Rect],
    ) -> Result<()> {
        let size = capture.texels.size;
        let input = self.pool_input(
            capture.pool_index,
            Rect::new(0.0, 0.0, size.width, size.height),
        );
        let mut turned = std::mem::take(&mut self.scratch.turned);
        turned.clear();
        turned.extend(scissors.iter().map(|rect| frame_target.texels(*rect)));
        self.defer_barrier(Some(
            frame_target.barrier(ImageAccess::COLOR_ATTACHMENT, ImageAccess::COLOR_ATTACHMENT),
        ));
        let result = self.record_pass(
            ctx,
            Recorded {
                label: "rikka.fx.mix",
                shader: &COMPOSITE_SHADER,
                params: mapped(Transform::Normal),
                input,
                src_scene: None,
                output: PassOutput {
                    view: frame_target.view,
                    format: frame_target.format,
                    region: capture.texels,
                },
                load_op: vk::AttachmentLoadOp::LOAD,
                composite: Composite::Mixed(mix.clamp(0.0, 1.0)),
                scissors: &turned,
            },
        );
        self.scratch.turned = turned;
        self.unsettled(result)
    }

    pub(crate) fn release_capture(&mut self, capture: Capture) {
        self.pool.release(capture.pool_index);
    }
}
