use ash::vk;
use smallvec::SmallVec;

use crate::error::Result;
use crate::fluid::FluidFrame;
use crate::frame_data::FrameAddresses;
use crate::graph::builder::{FrameGraph, GraphScratch, PassKind, PassScope, Rendering};
use crate::graph::compile::Transition;
use crate::graph::resource::{ImageId, Import, Load};
use crate::graph::store::{ImageStore, ViewGraph};
use crate::record::Record;
use crate::shadow::ShadowAtlas;
use crate::viewport::{Viewport, ViewportImages};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::GpuImage;
use rikka_gpu::texture::{TextureId, TexturePool};

const UNKNOWN_HISTORY: (vk::PipelineStageFlags2, vk::AccessFlags2) = (
    vk::PipelineStageFlags2::ALL_COMMANDS,
    vk::AccessFlags2::MEMORY_WRITE,
);

struct Holders<'x> {
    shared: &'x mut ImageStore,
    view: &'x mut ViewportImages,
    shadow: &'x mut Option<ShadowAtlas>,
    textures: &'x mut TexturePool,
    output: TextureId,
}

fn image_mut<'x>(
    state: &'x mut ViewGraph,
    holders: &'x mut Holders<'_>,
    image: ImageId,
) -> Option<&'x mut GpuImage> {
    let Some(import) = state.import_of(image) else {
        return state.physical_mut(image, holders.shared);
    };
    let view = &mut *holders.view;
    match import {
        Import::ColorTarget => Some(view.color_target()),
        Import::Resolved => Some(&mut view.resolved),
        Import::DepthTarget => Some(view.depth_target()),
        Import::DepthResolved => Some(&mut view.depth_resolved),
        Import::IdentityTarget => view.identity_target(),
        Import::Identity => view.identity_resolved.as_mut(),
        Import::Outline => view.outline.as_mut(),
        Import::Hzb => view.hzb.as_mut().map(|hzb| &mut hzb.image),
        Import::Output => holders.textures.image_mut(holders.output),
        Import::ShadowAtlas => holders.shadow.as_mut().map(|atlas| &mut atlas.image),
        Import::ShadowTint => holders
            .shadow
            .as_mut()
            .and_then(|atlas| atlas.tint.as_mut())
            .map(|tint| &mut tint.color.image),
        Import::ShadowTintDepth => holders
            .shadow
            .as_mut()
            .and_then(|atlas| atlas.tint.as_mut())
            .map(|tint| &mut tint.depth.image),
        Import::FigureAtlas => holders
            .shadow
            .as_mut()
            .and_then(|atlas| atlas.figures.as_mut())
            .map(|figures| &mut figures.image.image),
    }
}

fn transition(
    state: &mut ViewGraph,
    holders: &mut Holders<'_>,
    planned: Transition,
) -> Option<vk::ImageMemoryBarrier2<'static>> {
    let bound = state.bound(planned.image);
    let access = planned.usage.access(planned.stage, bound.layout);
    let image = image_mut(state, holders, planned.image)?;
    if planned.discard {
        image.discard();
    }
    let mut barrier = image.transition(access)?;
    if planned.first
        && let Some(alias) = bound.alias
    {
        let (stage, access) = match bound.fresh {
            true => UNKNOWN_HISTORY,
            false => (alias.stage, alias.access),
        };
        barrier.src_stage_mask |= stage;
        barrier.src_access_mask |= access;
    }
    Some(barrier)
}

fn record_barriers(
    ctx: &GpuFrameCtx<'_>,
    images: &[vk::ImageMemoryBarrier2<'static>],
    memory: Option<(
        rikka_gpu::core::barrier::MemoryAccess,
        rikka_gpu::core::barrier::MemoryAccess,
    )>,
) {
    let memory: SmallVec<[vk::MemoryBarrier2<'static>; 1]> = memory
        .map(|(from, to)| {
            vk::MemoryBarrier2::default()
                .src_stage_mask(from.stage)
                .src_access_mask(from.access)
                .dst_stage_mask(to.stage)
                .dst_access_mask(to.access)
        })
        .into_iter()
        .collect();
    if images.is_empty() && memory.is_empty() {
        return;
    }
    let dependency = vk::DependencyInfo::default()
        .image_memory_barriers(images)
        .memory_barriers(&memory);
    unsafe {
        ctx.device
            .handle()
            .cmd_pipeline_barrier2(ctx.cmd, &dependency)
    };
}

fn begin_rendering(
    record: &mut Record<'_>,
    ctx: &GpuFrameCtx<'_>,
    state: &ViewGraph,
    rendering: &Rendering,
) {
    let attachment = |image: ImageId,
                      load: Load,
                      store: bool,
                      resolve: Option<(ImageId, vk::ResolveModeFlags)>,
                      layout: vk::ImageLayout| {
        let mut info = vk::RenderingAttachmentInfo::default()
            .image_view(state.bound(image).view)
            .image_layout(layout)
            .load_op(load.op())
            .store_op(match store {
                true => vk::AttachmentStoreOp::STORE,
                false => vk::AttachmentStoreOp::DONT_CARE,
            });
        if let Load::Clear(value) = load {
            info = info.clear_value(value);
        }
        if let Some((target, mode)) = resolve {
            info = info
                .resolve_mode(mode)
                .resolve_image_view(state.bound(target).view)
                .resolve_image_layout(layout);
        }
        info
    };
    let colors: SmallVec<[vk::RenderingAttachmentInfo<'static>; 2]> = rendering
        .colors
        .iter()
        .map(|color| {
            attachment(
                color.image,
                color.load,
                color.store,
                color.resolve,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            )
        })
        .collect();
    let depth = rendering.depth.map(|depth| {
        attachment(
            depth.image,
            depth.load,
            depth.store,
            depth.resolve,
            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
        )
    });
    let first = rendering
        .colors
        .first()
        .map(|color| color.image)
        .or(rendering.depth.map(|depth| depth.image))
        .expect("a render pass has an attachment");
    let extent = state.bound(first).extent;
    let area = rendering.area.unwrap_or(vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent,
    });
    record.begin_scope(ctx, extent, area, &colors, depth.as_ref());
}

pub(crate) fn execute(
    mut graph: FrameGraph<'_>,
    record: &mut Record<'_>,
    ctx: &mut GpuFrameCtx<'_>,
    viewport: &mut Viewport,
    addresses: &FrameAddresses,
    fluid: Option<&FluidFrame>,
) -> Result<GraphScratch> {
    let output = viewport.output;
    let view = &mut viewport
        .targets
        .as_mut()
        .expect("viewport targets exist until destroy")
        .images;
    let state = &mut viewport.graph;
    let plan = state
        .plan
        .take()
        .expect("the graph is realized before execution");
    let mut barriers = std::mem::take(&mut state.barriers);
    let mut steps = plan.steps.iter().peekable();
    let mut open: Option<&'static str> = None;
    let mut outcome = Ok(());
    for index in 0..graph.recorders.len() {
        let Some(step) = steps.next_if(|step| step.pass == index) else {
            continue;
        };
        let pass = &graph.passes[index];
        if pass.group != open {
            if open.is_some() {
                ctx.end_pass();
            }
            if let Some(group) = pass.group {
                ctx.begin_pass(group);
            }
            open = pass.group;
        }
        transitions(
            state,
            record,
            view,
            output,
            ctx,
            &mut barriers,
            &plan.transitions[step.before.clone()],
        );
        record_barriers(ctx, &barriers, step.memory);
        barriers.clear();
        let scope = PassScope {
            images: &state.bound,
            nodes: &graph.images,
            accesses: &graph.image_accesses[pass.images.clone()],
            addresses,
            fluid,
        };
        ctx.begin_pass(pass.name);
        if let PassKind::Graphics(rendering) = &pass.kind {
            begin_rendering(record, ctx, state, rendering);
        }
        record.forget_bindings();
        let result = graph.recorders.run(index, record, ctx, &scope);
        if let PassKind::Graphics(_) = &pass.kind {
            record.end_scope(ctx);
        }
        ctx.end_pass();
        if let Err(error) = result {
            outcome = Err(error);
            break;
        }
        transitions(
            state,
            record,
            view,
            output,
            ctx,
            &mut barriers,
            &plan.transitions[step.after.clone()],
        );
    }
    record_barriers(ctx, &barriers, None);
    if open.is_some() {
        ctx.end_pass();
    }
    barriers.clear();
    state.barriers = barriers;
    state.plan = Some(plan);
    outcome.map(|()| GraphScratch::reclaim(graph))
}

fn transitions(
    state: &mut ViewGraph,
    record: &mut Record<'_>,
    view: &mut ViewportImages,
    output: TextureId,
    ctx: &GpuFrameCtx<'_>,
    pending: &mut Vec<vk::ImageMemoryBarrier2<'static>>,
    planned: &[Transition],
) {
    let mut holders = Holders {
        shared: &mut record.mesh.look_state.images,
        view,
        shadow: &mut record.mesh.shadow,
        textures: &mut *record.textures,
        output,
    };
    for &planned in planned {
        let Some(barrier) = transition(state, &mut holders, planned) else {
            continue;
        };
        if pending.iter().any(|known| known.image == barrier.image) {
            record_barriers(ctx, pending, None);
            pending.clear();
        }
        pending.push(barrier);
    }
}
