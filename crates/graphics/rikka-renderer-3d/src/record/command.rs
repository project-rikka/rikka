use ash::vk;
use bytemuck::Pod;

use crate::error::Result;
use crate::frame_data::{COUNTER_BUCKETS, FrameAddresses};
use crate::gpu::{GpuBucket, GpuDrawCommand, GpuPassPush, element_address};
use crate::mesh::IndexWidth;
use crate::pipeline::{DrawState, Recipe};
use crate::recipes;
use crate::record::Record;
use crate::shader::MaterialShader;
use crate::shadow::{self, ShadowView};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::pipeline::Blend;
use rikka_spirv::Shader;

#[derive(Default)]
pub(super) struct BindState {
    pipeline: vk::Pipeline,
    draw: Option<DrawState>,
    push: Option<[u8; size_of::<GpuPassPush>()]>,
    indices: Option<IndexWidth>,
}

impl Record<'_> {
    pub(crate) fn pipeline(&mut self, recipe: &Recipe) -> Result<vk::Pipeline> {
        if self.clipping && recipe.clips {
            let clipped = recipe.clipped(&self.mesh.builtin.clip_frag);
            return Ok(self.mesh.pipelines.get(&clipped)?);
        }
        Ok(self.mesh.pipelines.get(recipe)?)
    }

    pub(crate) fn bind(&mut self, ctx: &GpuFrameCtx<'_>, pipeline: vk::Pipeline, draw: DrawState) {
        let device = ctx.device.handle();
        if self.bound.pipeline != pipeline {
            unsafe { device.cmd_bind_pipeline(ctx.cmd, vk::PipelineBindPoint::GRAPHICS, pipeline) };
            self.bound.pipeline = pipeline;
        }
        if self.bound.draw != Some(draw) {
            draw.record(device, ctx.cmd);
            self.bound.draw = Some(draw);
        }
    }

    pub(crate) fn push(&mut self, ctx: &GpuFrameCtx<'_>, push: &GpuPassPush) {
        let bytes: [u8; size_of::<GpuPassPush>()] = bytemuck::cast(*push);
        if self.bound.push == Some(bytes) {
            return;
        }
        push_constants(ctx, self.mesh.pipelines.layout(), &bytes);
        self.bound.push = Some(bytes);
    }

    pub(crate) fn push_bytes(&mut self, ctx: &GpuFrameCtx<'_>, bytes: &[u8]) {
        push_constants(ctx, self.mesh.pipelines.layout(), bytes);
        self.bound.push = None;
    }

    fn bind_indices(&mut self, ctx: &GpuFrameCtx<'_>, width: IndexWidth) {
        if self.bound.indices == Some(width) {
            return;
        }
        unsafe {
            ctx.device.handle().cmd_bind_index_buffer(
                ctx.cmd,
                self.mesh.meshes.index_buffer(width),
                0,
                width.index_type(),
            );
        }
        self.bound.indices = Some(width);
    }

    pub(crate) fn forget_bindings(&mut self) {
        self.bound = BindState::default();
    }

    pub(crate) fn draw_range(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        addresses: &FrameAddresses,
        bucket: u32,
        at: GpuBucket,
        indices: IndexWidth,
    ) {
        if at.capacity == 0 {
            return;
        }
        self.bind_indices(ctx, indices);
        let commands_offset = element_address::<GpuDrawCommand>(0, at.first);
        let count_offset = element_address::<u32>(0, COUNTER_BUCKETS + bucket);
        unsafe {
            ctx.device.handle().cmd_draw_indexed_indirect_count(
                ctx.cmd,
                addresses.commands_buffer,
                commands_offset,
                addresses.counters_buffer,
                count_offset,
                at.capacity,
                size_of::<GpuDrawCommand>() as u32,
            );
        }
    }

    pub(crate) fn draw_indexed(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        index_count: u32,
        first_index: u32,
        vertex_offset: i32,
        instance: u32,
        indices: IndexWidth,
    ) {
        self.bind_indices(ctx, indices);
        unsafe {
            ctx.device.handle().cmd_draw_indexed(
                ctx.cmd,
                index_count,
                1,
                first_index,
                vertex_offset,
                instance,
            );
        }
    }

    pub(crate) fn begin_scope(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        extent: vk::Extent2D,
        area: vk::Rect2D,
        color: &[vk::RenderingAttachmentInfo<'_>],
        depth: Option<&vk::RenderingAttachmentInfo<'_>>,
    ) {
        let mut info = vk::RenderingInfo::default()
            .render_area(area)
            .layer_count(1)
            .color_attachments(color);
        if let Some(depth) = depth {
            info = info.depth_attachment(depth);
        }
        unsafe { ctx.device.handle().cmd_begin_rendering(ctx.cmd, &info) };
        set_viewport(ctx, extent, area);
    }

    pub(crate) fn end_scope(&mut self, ctx: &GpuFrameCtx<'_>) {
        unsafe { ctx.device.handle().cmd_end_rendering(ctx.cmd) };
    }

    pub(crate) fn dispatch_kernel<P: Pod>(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        kernel: Shader,
        push: &P,
        (x, y): (u32, u32),
    ) -> Result<()> {
        let pipeline = self.mesh.pipelines.kernel(kernel)?;
        bind_compute(ctx, pipeline);
        push_constants(ctx, self.mesh.pipelines.layout(), bytemuck::bytes_of(push));
        dispatch(ctx, x, y, 1);
        self.forget_bindings();
        Ok(())
    }

    pub(crate) fn dispatch_pipeline(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        pipeline: vk::Pipeline,
        push: &[u8],
        (x, y): (u32, u32),
    ) {
        bind_compute(ctx, pipeline);
        push_constants(ctx, self.mesh.pipelines.layout(), push);
        dispatch(ctx, x, y, 1);
        self.forget_bindings();
    }

    pub(crate) fn effect_fullscreen(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        shader: &MaterialShader,
        format: vk::Format,
        push: &[u8],
    ) -> Result<()> {
        let recipe = recipes::filter(&self.mesh.builtin, shader, Blend::Opaque, format);
        self.fullscreen(ctx, &recipe, DrawState::NO_DEPTH, push)
    }

    pub(crate) fn fullscreen(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        recipe: &Recipe,
        draw: DrawState,
        push: &[u8],
    ) -> Result<()> {
        let pipeline = self.pipeline(recipe)?;
        self.bind(ctx, pipeline, draw);
        push_constants(ctx, self.mesh.pipelines.layout(), push);
        self.bound.push = None;
        unsafe { ctx.device.handle().cmd_draw(ctx.cmd, 3, 1, 0, 0) };
        Ok(())
    }
}

pub(crate) fn bind_table(ctx: &GpuFrameCtx<'_>, layout: vk::PipelineLayout) {
    let sets = [ctx.resources.set()];
    unsafe {
        let device = ctx.device.handle();
        device.cmd_bind_descriptor_sets(
            ctx.cmd,
            vk::PipelineBindPoint::GRAPHICS,
            layout,
            0,
            &sets,
            &[],
        );
        device.cmd_bind_descriptor_sets(
            ctx.cmd,
            vk::PipelineBindPoint::COMPUTE,
            layout,
            0,
            &sets,
            &[],
        );
    }
}

pub(crate) fn push_constants(ctx: &GpuFrameCtx<'_>, layout: vk::PipelineLayout, bytes: &[u8]) {
    unsafe {
        ctx.device.handle().cmd_push_constants(
            ctx.cmd,
            layout,
            vk::ShaderStageFlags::VERTEX
                | vk::ShaderStageFlags::FRAGMENT
                | vk::ShaderStageFlags::COMPUTE,
            0,
            bytes,
        );
    }
}

pub(crate) fn bind_compute(ctx: &GpuFrameCtx<'_>, pipeline: vk::Pipeline) {
    unsafe {
        ctx.device
            .handle()
            .cmd_bind_pipeline(ctx.cmd, vk::PipelineBindPoint::COMPUTE, pipeline);
    }
}

pub(crate) fn dispatch(ctx: &GpuFrameCtx<'_>, x: u32, y: u32, z: u32) {
    unsafe { ctx.device.handle().cmd_dispatch(ctx.cmd, x, y, z) };
}

fn set_viewport(ctx: &GpuFrameCtx<'_>, extent: vk::Extent2D, scissor: vk::Rect2D) {
    let viewport = vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: extent.width as f32,
        height: extent.height as f32,
        min_depth: 0.0,
        max_depth: 1.0,
    };
    unsafe {
        ctx.device
            .handle()
            .cmd_set_viewport(ctx.cmd, 0, &[viewport]);
        ctx.device.handle().cmd_set_scissor(ctx.cmd, 0, &[scissor]);
    }
}

pub(crate) fn set_tile(ctx: &GpuFrameCtx<'_>, view: &ShadowView, divisor: u32) {
    let (origin, size) = shadow::tile_viewport(view);
    let (origin, size) = (origin / divisor as f32, size / divisor as f32);
    let viewport = vk::Viewport {
        x: origin.x,
        y: origin.y,
        width: size.x,
        height: size.y,
        min_depth: 0.0,
        max_depth: 1.0,
    };
    let scissor = vk::Rect2D {
        offset: vk::Offset2D {
            x: origin.x as i32,
            y: origin.y as i32,
        },
        extent: vk::Extent2D {
            width: size.x as u32,
            height: size.y as u32,
        },
    };
    unsafe {
        ctx.device
            .handle()
            .cmd_set_viewport(ctx.cmd, 0, &[viewport]);
        ctx.device.handle().cmd_set_scissor(ctx.cmd, 0, &[scissor]);
    }
}

pub(crate) fn clear_tile(
    ctx: &GpuFrameCtx<'_>,
    view: &ShadowView,
    divisor: u32,
    attachment: vk::ClearAttachment,
) {
    let (origin, size) = shadow::tile_viewport(view);
    let (origin, size) = (origin / divisor as f32, size / divisor as f32);
    let rect = vk::ClearRect {
        rect: vk::Rect2D {
            offset: vk::Offset2D {
                x: origin.x as i32,
                y: origin.y as i32,
            },
            extent: vk::Extent2D {
                width: size.x as u32,
                height: size.y as u32,
            },
        },
        base_array_layer: 0,
        layer_count: 1,
    };
    unsafe {
        ctx.device
            .handle()
            .cmd_clear_attachments(ctx.cmd, &[attachment], &[rect]);
    }
}
