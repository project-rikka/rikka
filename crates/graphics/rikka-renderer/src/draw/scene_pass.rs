use ash::vk;
use rikka_colorimetry::{Description, Intent, Mapping};
use rikka_types::{Lens, Rect};

use crate::draw::batcher::Batch;
use crate::draw::chunks::{Chunk, Segment};
use crate::draw::occlusion::{Occluder, Occlusion, Piece, Position, ReblurRegion, cores_of};
use crate::draw::primitive_pipeline::{Glass, PrimitivePipeline};
use crate::draw::retained::{RetainedBatch, UNPLACED};
use crate::fx::executor::{Capture, FxExecutor};
use crate::fx::frame_target::FrameTarget;
use crate::scene::{FrameBackdrops, NodeId, Scene, Viewer};
use crate::transform::placement;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::geom::{rect_to_viewport, rect_to_vk};
use rikka_gpu::core::image::ImageAccess;
use rikka_gpu::error::Result;

pub(crate) struct ScenePass<'a> {
    pub primitive: &'a mut PrimitivePipeline,
    pub fx: &'a mut FxExecutor,
    pub occlusion: &'a mut Occlusion,
    pub backdrops: &'a FrameBackdrops,
    pub scene: &'a Scene,
    pub viewer: Viewer,
    pub batch: &'a Batch<'a>,
    pub instance_set: vk::DescriptorSet,
    pub table_set: vk::DescriptorSet,
    pub frame_target: FrameTarget,
}

impl<'a> ScenePass<'a> {
    pub(crate) fn record(&mut self, ctx: &mut GpuFrameCtx<'_>, damage: &[Rect]) -> Result<()> {
        let painted = self.paint(ctx, damage);
        if painted.is_err() {
            self.primitive.revert_uploads();
        }
        painted
    }

    fn paint(&mut self, ctx: &mut GpuFrameCtx<'_>, damage: &[Rect]) -> Result<()> {
        let Some(render_area) = damage.iter().copied().reduce(|a, b| a.union(&b)) else {
            return Ok(());
        };
        let batch: &'a Batch<'a> = self.batch;
        self.cut(damage);
        let pieces = std::mem::take(&mut self.occlusion.pieces);

        let mut scissor = Scissor::new(self.frame_target);
        let mut retained: Vec<Option<Capture>> = Vec::new();
        let all_occluded = !pieces.is_empty() && pieces.iter().all(|piece| piece.floor.is_some());
        let uncleared = all_occluded || self.frame_target.underlay;
        match (damage, uncleared) {
            (_, true) => self.begin(ctx, render_area, None),
            ([_], false) => self.begin(ctx, render_area, Some(self.background())),
            (_, false) => {
                self.begin(ctx, render_area, None);
                self.clear(ctx, &pieces);
            }
        }
        self.bind(ctx, &mut scissor)?;

        let mut aside = ScissorPieces {
            scissor: &mut scissor,
            damage,
            pieces: &pieces,
            render_area,
        };
        for (at, segment) in batch.segments.iter().enumerate() {
            match *segment {
                Segment::Draw { first, count } => {
                    let chunks = &batch.chunks[first as usize..(first + count) as usize];
                    self.draw_pieces(ctx, aside.scissor, (at, first), chunks, &pieces);
                }
                Segment::Backdrop {
                    node,
                    instance,
                    region,
                } => self.draw_backdrop(
                    ctx,
                    &mut aside,
                    Backdrop {
                        node,
                        instance,
                        region,
                        at,
                    },
                )?,
                Segment::Capture { region } => {
                    let taken = self.capture(ctx, &mut aside, region)?;
                    retained.push(taken);
                }
                Segment::Fade { opacity, .. } => {
                    if let Some(taken) = retained.pop().flatten() {
                        self.mix_back(ctx, &mut aside, taken, opacity)?;
                    }
                }
            }
        }

        self.end(ctx);
        self.occlusion.pieces = pieces;
        Ok(())
    }

    fn cut(&mut self, damage: &[Rect]) {
        let occlusion = &mut *self.occlusion;
        occlusion.clear();
        let scene: &Scene = self.scene;
        if let Some(retained) = self.batch.retained
            && !retained.occluders.is_empty()
        {
            gather(
                occlusion,
                OccluderQuery {
                    scene,
                    viewer: self.viewer,
                    retained,
                    backdrops: self.backdrops,
                    segments: self.batch.segments,
                    chunks: self.batch.chunks,
                    fx: self.fx,
                    viewport: self.frame_target.scene,
                    damage,
                },
            );
        }
        occlusion.cut(damage);
    }

    fn begin(
        &self,
        ctx: &mut GpuFrameCtx<'_>,
        render_area: Rect,
        cleared_to: Option<vk::ClearValue>,
    ) {
        ctx.begin_pass("rikka.scene.draw");
        let attachment = vk::RenderingAttachmentInfo::default()
            .image_view(self.frame_target.view)
            .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .store_op(vk::AttachmentStoreOp::STORE);
        let attachment = match cleared_to {
            Some(value) => attachment
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .clear_value(value),
            None => attachment.load_op(vk::AttachmentLoadOp::LOAD),
        };
        let attachments = [attachment];
        let info = vk::RenderingInfo::default()
            .render_area(rect_to_vk(self.frame_target.texels(render_area)))
            .layer_count(1)
            .color_attachments(&attachments);
        unsafe { ctx.device.handle().cmd_begin_rendering(ctx.cmd, &info) };
    }

    fn end(&self, ctx: &mut GpuFrameCtx<'_>) {
        unsafe { ctx.device.handle().cmd_end_rendering(ctx.cmd) };
        ctx.end_pass();
    }

    fn background(&self) -> vk::ClearValue {
        let background = self.scene.background();
        let [red, green, blue] = Mapping::new(
            &Description::SRGB,
            Intent::Perceptual,
            &self.frame_target.space,
        )
        .expect("sRGB maps into every blend space")
        .apply([background.r, background.g, background.b].map(f64::from))
        .map(|channel| channel as f32 * background.a);
        vk::ClearValue {
            color: vk::ClearColorValue {
                float32: [red, green, blue, background.a],
            },
        }
    }

    fn clear(&mut self, ctx: &GpuFrameCtx<'_>, pieces: &[Piece]) {
        let clear_attachment = vk::ClearAttachment::default()
            .aspect_mask(vk::ImageAspectFlags::COLOR)
            .color_attachment(0)
            .clear_value(self.background());
        let clear_rects = &mut self.occlusion.clear_rects;
        clear_rects.clear();
        clear_rects.extend(
            pieces
                .iter()
                .filter(|piece| piece.floor.is_none())
                .map(|piece| vk::ClearRect {
                    rect: rect_to_vk(self.frame_target.texels(piece.rect)),
                    base_array_layer: 0,
                    layer_count: 1,
                }),
        );
        if clear_rects.is_empty() {
            return;
        }
        unsafe {
            ctx.device
                .handle()
                .cmd_clear_attachments(ctx.cmd, &[clear_attachment], clear_rects);
        }
    }

    fn bind(&mut self, ctx: &GpuFrameCtx<'_>, scissor: &mut Scissor) -> Result<()> {
        let scene = self.frame_target.scene;
        scissor.forget();
        self.primitive.bind(
            ctx.cmd,
            self.frame_target.format,
            self.instance_set,
            self.table_set,
            [scene.width(), scene.height()],
            placement(self.frame_target.turn),
        )?;
        unsafe {
            ctx.device.handle().cmd_set_viewport(
                ctx.cmd,
                0,
                &[rect_to_viewport(self.frame_target.texels(scene))],
            );
        }
        Ok(())
    }

    fn draw_pieces(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        scissor: &mut Scissor,
        (segment, first_chunk): (usize, u32),
        chunks: &[Chunk],
        pieces: &[Piece],
    ) {
        for piece in pieces {
            let floor = match piece.floor {
                Some(floor) if floor.segment as usize > segment => continue,
                Some(floor) if floor.segment as usize == segment => Some(floor),
                _ => None,
            };
            let mut run: Option<(u32, u32)> = None;
            for (offset, chunk) in chunks.iter().enumerate() {
                let index = first_chunk + offset as u32;
                let frame_target = match floor {
                    Some(floor) if index < floor.chunk => continue,
                    Some(floor) if index == floor.chunk => {
                        (floor.instance, chunk.first + chunk.count - floor.instance)
                    }
                    _ => (chunk.first, chunk.count),
                };
                if !chunk.bounds.intersects(&piece.rect) {
                    if let Some((from, count)) = run.take() {
                        self.primitive.draw(ctx.cmd, from, count);
                    }
                    continue;
                }
                scissor.set(ctx, piece.rect);
                run = match run {
                    Some((from, count)) if from + count == frame_target.0 => {
                        Some((from, count + frame_target.1))
                    }
                    Some((from, count)) => {
                        self.primitive.draw(ctx.cmd, from, count);
                        Some(frame_target)
                    }
                    None => Some(frame_target),
                };
            }
            if let Some((from, count)) = run {
                self.primitive.draw(ctx.cmd, from, count);
            }
        }
    }

    fn draw_backdrop(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        aside: &mut ScissorPieces<'_>,
        backdrop: Backdrop,
    ) -> Result<()> {
        let scene: &'a Scene = self.scene;
        let Some(node) = scene.get(backdrop.node) else {
            return Ok(());
        };
        let painted = scene.bounds_of(node);
        let at = Position::segment(backdrop.at);
        let shown = |piece: &&Piece| piece.draws(at) && piece.rect.intersects(&painted);
        let prepared = self.backdrops.get(backdrop.node);
        let cache = scene.backdrop_cache(self.viewer, backdrop.node);
        if !aside.pieces.iter().any(|piece| shown(&piece)) {
            if let Some(cache) = cache {
                let pending = !cache.stale.is_empty()
                    || prepared
                        .and_then(|entry| entry.reshaped)
                        .is_some_and(|corners| !corners.is_empty());
                if pending {
                    self.fx.invalidate(cache.key);
                }
            }
            return Ok(());
        }
        let fallback_plan;
        let plan = match prepared {
            Some(entry) => &entry.plan,
            None => {
                let recipe = node
                    .recipe(scene.offset_of(node.group))
                    .expect("backdrop segment refers to a backdrop node");
                fallback_plan = recipe.plan(self.frame_target.scene);
                &fallback_plan
            }
        };

        let retained = cache.and_then(|cache| self.fx.reusable(plan, cache));
        let output = match retained {
            Some(output) => Some(output),
            None => {
                self.end(ctx);
                ctx.begin_pass("rikka.scene.backdrop");
                let frame_target = self.frame_target;
                let output = self.fx.run_effect(ctx, plan, &frame_target, cache)?;
                ctx.end_pass();
                self.take_up(ctx, aside)?;
                output
            }
        };

        let glass = output.as_ref().map(|out| Glass {
            tex: out.tex,
            uv: out.uv_of(backdrop.region),
            bounds: out.uv_bounds(),
            adapt: node.lens.as_deref().copied().map_or(0.0, Lens::adapt),
        });
        self.primitive.set_glass(ctx.cmd, glass);

        for piece in aside.pieces.iter().filter(shown) {
            if let Some(clip) = piece.rect.intersection(&painted) {
                aside.scissor.set(ctx, clip);
                self.primitive.draw(ctx.cmd, backdrop.instance, 1);
            }
        }
        if let Some(out) = output {
            self.fx.release_output(out);
        }
        Ok(())
    }

    fn capture(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        aside: &mut ScissorPieces<'_>,
        region: Rect,
    ) -> Result<Option<Capture>> {
        let region = region.round_out();
        let Some(taken) = aside
            .damage
            .iter()
            .filter_map(|rect| rect.intersection(&region))
            .reduce(|all, rect| all.union(&rect))
            .and_then(|taken| taken.round_out().intersection(&self.frame_target.scene))
        else {
            return Ok(None);
        };
        self.end(ctx);
        ctx.begin_pass("rikka.scene.capture");
        let frame_target = self.frame_target;
        let capture = self.fx.capture(ctx, &frame_target, taken);
        ctx.end_pass();
        let capture = capture?;
        self.take_up(ctx, aside)?;
        Ok(Some(capture))
    }

    fn mix_back(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        aside: &mut ScissorPieces<'_>,
        capture: Capture,
        opacity: f32,
    ) -> Result<()> {
        self.end(ctx);
        ctx.begin_pass("rikka.scene.fade");
        let frame_target = self.frame_target;
        let mixed = self
            .fx
            .mix_back(ctx, &frame_target, &capture, opacity, aside.damage);
        ctx.end_pass();
        self.fx.release_capture(capture);
        mixed?;
        self.fx.defer_barrier(Some(
            self.frame_target
                .barrier(ImageAccess::COLOR_ATTACHMENT, ImageAccess::COLOR_ATTACHMENT),
        ));
        self.take_up(ctx, aside)
    }

    fn take_up(&mut self, ctx: &mut GpuFrameCtx<'_>, aside: &mut ScissorPieces<'_>) -> Result<()> {
        self.fx.settle(ctx);
        self.begin(ctx, aside.render_area, None);
        self.bind(ctx, aside.scissor)
    }
}

struct OccluderQuery<'g> {
    scene: &'g Scene,
    viewer: Viewer,
    retained: &'g RetainedBatch,
    backdrops: &'g FrameBackdrops,
    segments: &'g [Segment],
    chunks: &'g [Chunk],
    fx: &'g FxExecutor,
    viewport: Rect,
    damage: &'g [Rect],
}

impl OccluderQuery<'_> {
    fn touches(&self, rect: &Rect) -> bool {
        self.damage.iter().any(|damage| damage.intersects(rect))
    }
}

fn gather(occlusion: &mut Occlusion, gathering: OccluderQuery<'_>) {
    let OccluderQuery {
        scene,
        viewer,
        retained,
        backdrops,
        segments,
        chunks,
        fx,
        viewport,
        ..
    } = gathering;
    let opaque_ground = scene.background().a >= 1.0;
    for (at, segment) in segments.iter().enumerate() {
        match *segment {
            Segment::Draw { first, count } => {
                occlusion.draws.push((first, at as u32));
                for index in first..first + count {
                    let chunk = chunks[index as usize];
                    if chunk.has_textured && gathering.touches(&chunk.bounds) {
                        occlusion.beneath.push((
                            Position {
                                segment: at as u32,
                                chunk: index,
                                instance: chunk.first,
                            },
                            chunk.bounds,
                        ));
                    }
                }
            }
            Segment::Backdrop { node: id, .. } => {
                let Some(node) = scene.get(id) else { continue };
                let painted = scene.bounds_of(node);
                if !gathering.touches(&painted) {
                    continue;
                }
                occlusion.beneath.push((Position::segment(at), painted));
                let cache = scene.backdrop_cache(viewer, id);
                let (reads, reused) = match backdrops.get(id) {
                    Some(entry) => {
                        let reused = cache.is_some_and(|cache| cache.stale.is_empty())
                            && entry.reshaped.is_some_and(|reshaped| reshaped.is_empty());
                        (entry.plan.region, reused)
                    }
                    None => {
                        let Some(plan) = node
                            .recipe(scene.offset_of(node.group))
                            .map(|recipe| recipe.plan(viewport))
                        else {
                            continue;
                        };
                        let reused = cache.is_some_and(|cache| fx.matches(&plan, cache));
                        (plan.region, reused)
                    }
                };
                if !reused {
                    occlusion.reblurred.push(ReblurRegion {
                        segment: at as u32,
                        reads,
                    });
                    continue;
                }
                if opaque_ground
                    && occludes(retained, id)
                    && let Some(cores) = cores_of(scene, node)
                {
                    occlusion.occluders.push(Occluder {
                        position: Position::segment(at),
                        cores,
                    });
                }
            }
            _ => {}
        }
    }
    for &id in &retained.occluders {
        let Some(slot) = retained
            .holds(id)
            .filter(|slot| !slot.is_glass && slot.span.count > 0)
        else {
            continue;
        };
        if slot.chunk == UNPLACED {
            continue;
        }
        let (Some(node), Some(chunk)) = (scene.get(id), retained.chunk_index(slot.chunk)) else {
            continue;
        };
        let Some(cores) = cores_of(scene, node) else {
            continue;
        };
        if !cores.iter().any(|core| gathering.touches(core)) {
            continue;
        }
        let draws = &occlusion.draws;
        let at = draws.partition_point(|(first, _)| *first as usize <= chunk);
        let Some(&(_, segment)) = at.checked_sub(1).and_then(|at| draws.get(at)) else {
            continue;
        };
        occlusion.occluders.push(Occluder {
            position: Position {
                segment,
                chunk: chunk as u32,
                instance: slot.span.first,
            },
            cores,
        });
    }
    occlusion.keep_worthwhile();
}

fn occludes(retained: &RetainedBatch, id: NodeId) -> bool {
    retained.holds(id).is_some_and(|slot| slot.occludes)
}

struct ScissorPieces<'a> {
    scissor: &'a mut Scissor,
    damage: &'a [Rect],
    pieces: &'a [Piece],
    render_area: Rect,
}

#[derive(Clone, Copy)]
struct Backdrop {
    node: NodeId,
    instance: u32,
    region: Rect,
    at: usize,
}

struct Scissor {
    frame_target: FrameTarget,
    last: Option<Rect>,
}

impl Scissor {
    fn new(frame_target: FrameTarget) -> Self {
        Self {
            frame_target,
            last: None,
        }
    }

    fn forget(&mut self) {
        self.last = None;
    }

    fn set(&mut self, ctx: &GpuFrameCtx<'_>, rect: Rect) {
        if self.last == Some(rect) {
            return;
        }
        self.last = Some(rect);
        unsafe {
            ctx.device.handle().cmd_set_scissor(
                ctx.cmd,
                0,
                &[rect_to_vk(self.frame_target.texels(rect))],
            )
        };
    }
}
