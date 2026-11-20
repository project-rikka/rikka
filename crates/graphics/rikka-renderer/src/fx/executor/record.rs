use ash::vk;
use rikka_types::{Rect, Transform};

use crate::fx::effect::{BackdropShader, PARAMS};
use crate::transform::unit_map;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::geom::{rect_to_viewport, rect_to_vk};
use rikka_gpu::core::pipeline::Blend;
use rikka_gpu::error::Result;

use super::FxExecutor;
use super::pass_input::{PassInput, PassOutput};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct FxPushConstants {
    src_rect: [f32; 4],
    src_texel: [f32; 2],
    src_per_dst: [f32; 2],
    params: [f32; PARAMS],
    src_origin: [f32; 2],
    src_scale: f32,
    src: u32,
}

const _: () = assert!(size_of::<FxPushConstants>() <= 128);

#[derive(Clone, Copy)]
pub(super) enum Composite {
    Over(Blend),
    Mixed(f32),
}

impl Composite {
    fn blend(self) -> Blend {
        match self {
            Self::Over(blend) => blend,
            Self::Mixed(_) => Blend::Mix,
        }
    }

    fn constants(self) -> Option<[f32; 4]> {
        match self {
            Self::Over(_) => None,
            Self::Mixed(mix) => Some([mix, mix, mix, mix]),
        }
    }
}

pub(super) struct Recorded<'a> {
    pub(super) label: &'static str,
    pub(super) shader: &'a BackdropShader,
    pub(super) params: [f32; PARAMS],
    pub(super) input: PassInput,
    pub(super) src_scene: Option<Rect>,
    pub(super) output: PassOutput,
    pub(super) load_op: vk::AttachmentLoadOp,
    pub(super) composite: Composite,
    pub(super) scissors: &'a [Rect],
}

pub(super) fn mapped(transform: Transform) -> [f32; PARAMS] {
    let (map, bias) = unit_map(transform);
    let mut params = [0.0f32; PARAMS];
    params[..4].copy_from_slice(&map);
    params[4..6].copy_from_slice(&bias);
    params
}

impl FxExecutor {
    pub(super) fn record_pass(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        pass: Recorded<'_>,
    ) -> Result<()> {
        let Recorded {
            label,
            shader,
            params,
            input,
            src_scene,
            output,
            load_op,
            composite,
            scissors,
        } = pass;
        let mut painted = scissors
            .iter()
            .filter_map(|scissor| scissor.intersection(&output.region));
        let Some(first) = painted.next() else {
            return Ok(());
        };
        let (painted, pieces) = painted.fold((first, 1), |(all, pieces), rect| {
            (all.union(&rect), pieces + 1)
        });
        let overwritten = pieces == 1 && painted == painted.round_out();
        let load_op = match composite {
            Composite::Over(Blend::Opaque) if overwritten => vk::AttachmentLoadOp::DONT_CARE,
            _ => load_op,
        };
        let handles = self
            .pipelines
            .get(shader, output.format, composite.blend())?;
        ctx.begin_pass(label);
        self.settle(ctx);

        let inv_w = 1.0 / input.texture_size.width as f32;
        let inv_h = 1.0 / input.texture_size.height as f32;
        let texels = match src_scene {
            Some(scene) => input.texels_of(scene),
            None => input.texels(),
        };
        let push = FxPushConstants {
            src_rect: [
                texels.min_x() * inv_w,
                texels.min_y() * inv_h,
                texels.max_x() * inv_w,
                texels.max_y() * inv_h,
            ],
            src_texel: [inv_w, inv_h],
            src_per_dst: [
                texels.width() / output.region.width().max(1.0),
                texels.height() / output.region.height().max(1.0),
            ],
            params,
            src_origin: [input.origin.x, input.origin.y],
            src_scale: input.scale,
            src: input.tex,
        };
        let color_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(output.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .load_op(load_op)
            .store_op(vk::AttachmentStoreOp::STORE);
        let color_attachments = [color_attachment];
        let render_area = rect_to_vk(painted);
        let rendering_info = vk::RenderingInfo::default()
            .render_area(render_area)
            .layer_count(1)
            .color_attachments(&color_attachments);

        let device = self.device.handle();
        unsafe {
            device.cmd_begin_rendering(ctx.cmd, &rendering_info);
            device.cmd_bind_pipeline(ctx.cmd, vk::PipelineBindPoint::GRAPHICS, handles.pipeline);
            device.cmd_bind_descriptor_sets(
                ctx.cmd,
                vk::PipelineBindPoint::GRAPHICS,
                handles.layout,
                0,
                &[ctx.resources.set()],
                &[],
            );
            device.cmd_push_constants(
                ctx.cmd,
                handles.layout,
                vk::ShaderStageFlags::FRAGMENT,
                0,
                bytemuck::bytes_of(&push),
            );
            if let Some(constants) = composite.constants() {
                device.cmd_set_blend_constants(ctx.cmd, &constants);
            }
            device.cmd_set_viewport(ctx.cmd, 0, &[rect_to_viewport(output.region)]);
            for &scissor in scissors {
                let Some(clipped) = scissor.intersection(&output.region) else {
                    continue;
                };
                device.cmd_set_scissor(ctx.cmd, 0, &[rect_to_vk(clipped)]);
                device.cmd_draw(ctx.cmd, 3, 1, 0, 0);
            }
            device.cmd_end_rendering(ctx.cmd);
        }
        ctx.end_pass();
        Ok(())
    }
}
