use ash::vk;
use bytemuck::{Pod, Zeroable};

use crate::build::BuiltFrame;
use crate::editor::{GRID_ITERATIONS, GridDraws, OUTLINE_COLORS};
use crate::error::Result;
use crate::frame_data::FrameAddresses;
use crate::graph::{
    Attachment, Buffer, BufferUse, FrameGraph, ImageId, ImageSpec, Load, SHADER_STAGES, Size,
};
use crate::material::Cull;
use crate::overlay::{
    GpuOverlayBatch, OverlayDraw, OverlayFrame, OverlayKind, OverlayLayer, OverlayPush,
};
use crate::pipeline::DrawState;
use crate::recipes::{self, OUTLINE_COLOR_FORMAT, OUTLINE_ID_FORMAT, OUTLINE_LINE_FORMAT};
use crate::record::Record;
use crate::record::compose::screen_area;
use crate::record::depth::{CLEAR_DEPTH, clear_depth};
use crate::record::direct::DirectPass;
use crate::record::stage::{SceneImages, Stage};
use crate::viewport::DEPTH_FORMAT;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const OVERLAY_DEPTH: &str = "rikka.three.overlay.depth";
const OCCLUDERS: &str = "rikka.three.overlay.occluders";
const OVERLAYS: &str = "rikka.three.overlay";
const OUTLINE_IDS: &str = "rikka.three.overlay.outline.ids";
const OUTLINE_DEPTH: &str = "rikka.three.overlay.outline.depth";
const OUTLINE_COLOR: &str = "rikka.three.overlay.outline.color";
const OUTLINE_LINES: &str = "rikka.three.overlay.outline.lines";
const OUTLINE_PREPASS: &str = "rikka.three.overlay.outline.prepass";
const OUTLINE_DETECT: &str = "rikka.three.overlay.outline.detect";
const OUTLINE_RESOLVE: &str = "rikka.three.overlay.outline.resolve";

const OUTLINE_MARGIN: u32 = 4;
const OUTLINE_THICK: u32 = 1;
const OUTLINE_SMOOTH: u32 = 2;
const OUTLINE_WIRES: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct EdgesPush {
    frame: u64,
    edges: u64,
    instance: u32,
    pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DetectPush {
    ids: u32,
    outline_depth: u32,
    scene_depth: u32,
    flags: u32,
    occluded_alpha: f32,
    pad: u32,
    size: [u32; 2],
    palette: [u32; OUTLINE_COLORS + 1],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ResolvePush {
    color: u32,
    segments: u32,
    smoothing: u32,
    line_kernel: f32,
    size: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GridPush {
    frame: u64,
    axes: u32,
    iteration: u32,
}

pub(super) fn active(built: &BuiltFrame, overlays: &OverlayFrame, grid: Option<GridDraws>) -> bool {
    !overlays.is_empty()
        || !built.outlined.is_empty()
        || !built.outline_edges.is_empty()
        || grid.is_some()
}

impl Record<'_> {
    pub(super) fn declare_overlays<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &SceneImages,
    ) {
        let overlays = stage.overlays;
        let built = stage.built;
        let grid = stage.grid;
        if !active(built, overlays, grid) {
            return;
        }
        let display = stage.view.display;
        let depth = graph.create(ImageSpec::new(
            OVERLAY_DEPTH,
            DEPTH_FORMAT,
            Size::Fixed(display.width, display.height),
        ));
        let outlined = !built.outlined.is_empty() || !built.outline_edges.is_empty();
        let occluded = overlays.tested() || outlined || grid.is_some();
        let load = match occluded {
            true => {
                graph
                    .render(OCCLUDERS)
                    .depth(Attachment::new(depth, clear_depth()))
                    .buffer(Buffer::Deform, BufferUse::storage_read(SHADER_STAGES))
                    .record(move |record, ctx, scope| {
                        record.draw_direct(
                            ctx,
                            built,
                            scope.addresses,
                            &built.occluders,
                            DirectPass::Occluder,
                        )
                    });
                Load::Keep
            }
            false => clear_depth(),
        };
        if outlined {
            declare_outline(graph, stage, scene, depth);
        }
        if overlays.is_empty() && grid.is_none() {
            return;
        }
        graph
            .render(OVERLAYS)
            .color(Attachment::new(scene.output, Load::Keep))
            .depth(Attachment::new(depth, load))
            .record(move |record, ctx, scope| {
                if let Some(grid) = grid {
                    record.draw_grid(ctx, scope.addresses, grid)?;
                }
                record.draw_overlays(ctx, scope.addresses, overlays, display, occluded)
            });
    }

    fn draw_grid(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        addresses: &FrameAddresses,
        grid: GridDraws,
    ) -> Result<()> {
        let pipeline = self.pipeline(&recipes::overlay(&self.mesh.builtin.editor_grid))?;
        self.bind(
            ctx,
            pipeline,
            DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER_OR_EQUAL, false),
        );
        for iteration in 0..grid.iterations.min(GRID_ITERATIONS) {
            for (axes, vertices) in [(1, grid.axis_vertices), (0, grid.grid_vertices)] {
                if vertices == 0 {
                    continue;
                }
                let push = GridPush {
                    frame: addresses.overlay_refs,
                    axes,
                    iteration,
                };
                self.push_bytes(ctx, bytemuck::bytes_of(&push));
                unsafe {
                    ctx.device.handle().cmd_draw(ctx.cmd, vertices, 1, 0, 0);
                }
            }
        }
        Ok(())
    }

    fn draw_outline_edges(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        built: &BuiltFrame,
        addresses: &FrameAddresses,
    ) -> Result<()> {
        if built.outline_edges.is_empty() {
            return Ok(());
        }
        let recipe = recipes::outline_edges(&self.mesh.builtin);
        let pipeline = self.pipeline(&recipe)?;
        self.bind(
            ctx,
            pipeline,
            DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER_OR_EQUAL, true),
        );
        for edges in &built.outline_edges {
            let push = EdgesPush {
                frame: addresses.overlay_refs,
                edges: edges.edges,
                instance: edges.instance,
                pad: 0,
            };
            self.push_bytes(ctx, bytemuck::bytes_of(&push));
            unsafe {
                ctx.device
                    .handle()
                    .cmd_draw(ctx.cmd, edges.count * 6, 1, 0, 0);
            }
        }
        Ok(())
    }

    fn draw_overlays(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        addresses: &FrameAddresses,
        overlays: &OverlayFrame,
        display: vk::Extent2D,
        occluded: bool,
    ) -> Result<()> {
        let builtin = &self.mesh.builtin;
        let pipelines = [
            recipes::overlay(&builtin.overlay_lines),
            recipes::overlay(&builtin.overlay_points),
            recipes::overlay(&builtin.overlay_triangles),
        ];
        let pipelines = [
            self.pipeline(&pipelines[0])?,
            self.pipeline(&pipelines[1])?,
            self.pipeline(&pipelines[2])?,
        ];
        for layer in [OverlayLayer::Scene, OverlayLayer::Front] {
            let mut drawn = overlays
                .draws()
                .iter()
                .filter(|draw| draw.layer == layer)
                .peekable();
            if layer == OverlayLayer::Front && drawn.peek().is_some() && occluded {
                clear_overlay_depth(ctx, display);
            }
            for draw in drawn {
                let pipeline = pipelines[match draw.kind {
                    OverlayKind::Lines => 0,
                    OverlayKind::Points => 1,
                    OverlayKind::Triangles => 2,
                }];
                let push = OverlayPush {
                    frame: addresses.overlay_refs,
                    batch: addresses.overlay_batches
                        + u64::from(draw.record) * size_of::<GpuOverlayBatch>() as u64,
                    alpha: 1.0,
                    pad: 0,
                };
                self.overlay_draw(ctx, pipeline, draw, vk::CompareOp::GREATER_OR_EQUAL, push);
                if let Some(alpha) = draw.faded {
                    let hidden = OverlayDraw {
                        writes_depth: false,
                        ..*draw
                    };
                    let push = OverlayPush { alpha, ..push };
                    self.overlay_draw(ctx, pipeline, &hidden, vk::CompareOp::LESS, push);
                }
            }
        }
        Ok(())
    }

    fn overlay_draw(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        pipeline: vk::Pipeline,
        draw: &OverlayDraw,
        compare: vk::CompareOp,
        push: OverlayPush,
    ) {
        let cull = match draw.cull {
            true => Cull::Back,
            false => Cull::None,
        };
        self.bind(
            ctx,
            pipeline,
            DrawState::depth_tested(cull, compare, draw.writes_depth),
        );
        self.push_bytes(ctx, bytemuck::bytes_of(&push));
        unsafe {
            ctx.device
                .handle()
                .cmd_draw(ctx.cmd, draw.vertices, 1, 0, 0);
        }
    }
}

fn declare_outline<'f>(
    graph: &mut FrameGraph<'f>,
    stage: Stage<'f>,
    scene: &SceneImages,
    scene_depth: ImageId,
) {
    let built = stage.built;
    let display = stage.view.display;
    let outlined = built
        .outlined
        .iter()
        .map(|draw| draw.instance)
        .chain(built.outline_edges.iter().map(|edges| edges.instance))
        .map(|instance| &built.instances[instance as usize]);
    let Some(area) = screen_area(
        stage.frame.camera.view_projection(),
        outlined,
        display,
        OUTLINE_MARGIN,
    ) else {
        return;
    };
    let detected = grown(area, 2, display);
    let size = Size::Fixed(display.width, display.height);
    let ids = graph.create(ImageSpec::new(OUTLINE_IDS, OUTLINE_ID_FORMAT, size).nearest());
    let outline_depth = graph.create(ImageSpec::new(OUTLINE_DEPTH, DEPTH_FORMAT, size).nearest());
    let color = graph.create(ImageSpec::new(OUTLINE_COLOR, OUTLINE_COLOR_FORMAT, size).nearest());
    let lines = graph.create(ImageSpec::new(OUTLINE_LINES, OUTLINE_LINE_FORMAT, size).nearest());
    let cleared = vk::ClearValue {
        color: vk::ClearColorValue { uint32: [0; 4] },
    };
    graph
        .render(OUTLINE_PREPASS)
        .color(Attachment::new(ids, Load::Clear(cleared)))
        .depth(Attachment::new(outline_depth, clear_depth()))
        .buffer(Buffer::Deform, BufferUse::storage_read(SHADER_STAGES))
        .record(move |record, ctx, scope| {
            record.draw_direct(
                ctx,
                built,
                scope.addresses,
                &built.outlined,
                DirectPass::Outline,
            )?;
            record.draw_outline_edges(ctx, built, scope.addresses)
        });
    let outline = stage.frame.editor.outline;
    let palette = built.outline_palette.packed();
    let flags = match outline.expands() {
        true => OUTLINE_THICK,
        false => 0,
    } | match outline.smooth {
        true => OUTLINE_SMOOTH,
        false => 0,
    } | match built.outline_edges.is_empty() {
        true => 0,
        false => OUTLINE_WIRES,
    };
    graph
        .render(OUTLINE_DETECT)
        .color(Attachment::new(color, Load::Clear(cleared)))
        .color(Attachment::new(lines, Load::Clear(cleared)))
        .area(detected)
        .read(ids)
        .read(outline_depth)
        .read(scene_depth)
        .record(move |record, ctx, scope| {
            let push = DetectPush {
                ids: scope.sampled(ids),
                outline_depth: scope.sampled(outline_depth),
                scene_depth: scope.sampled(scene_depth),
                flags,
                occluded_alpha: outline.occluded_alpha.clamp(0.0, 1.0),
                pad: 0,
                size: [display.width, display.height],
                palette,
            };
            record.fullscreen(
                ctx,
                &recipes::outline_detect(&record.mesh.builtin),
                DrawState::NO_DEPTH,
                bytemuck::bytes_of(&push),
            )
        });
    graph
        .render(OUTLINE_RESOLVE)
        .color(Attachment::new(scene.output, Load::Keep))
        .area(area)
        .read(color)
        .read(lines)
        .record(move |record, ctx, scope| {
            let push = ResolvePush {
                color: scope.sampled(color),
                segments: scope.sampled(lines),
                smoothing: u32::from(outline.smooth),
                line_kernel: outline.line_kernel(),
                size: [display.width, display.height],
            };
            record.fullscreen(
                ctx,
                &recipes::outline_resolve(&record.mesh.builtin),
                DrawState::NO_DEPTH,
                bytemuck::bytes_of(&push),
            )
        });
}

fn grown(area: vk::Rect2D, by: u32, display: vk::Extent2D) -> vk::Rect2D {
    let x = (area.offset.x - by as i32).max(0);
    let y = (area.offset.y - by as i32).max(0);
    let right = (area.offset.x + (area.extent.width + by) as i32).min(display.width as i32);
    let bottom = (area.offset.y + (area.extent.height + by) as i32).min(display.height as i32);
    vk::Rect2D {
        offset: vk::Offset2D { x, y },
        extent: vk::Extent2D {
            width: (right - x) as u32,
            height: (bottom - y) as u32,
        },
    }
}

fn clear_overlay_depth(ctx: &GpuFrameCtx<'_>, display: vk::Extent2D) {
    let attachment = vk::ClearAttachment {
        aspect_mask: vk::ImageAspectFlags::DEPTH,
        color_attachment: 0,
        clear_value: vk::ClearValue {
            depth_stencil: vk::ClearDepthStencilValue {
                depth: CLEAR_DEPTH,
                stencil: 0,
            },
        },
    };
    let rect = vk::ClearRect {
        rect: vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: display,
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
