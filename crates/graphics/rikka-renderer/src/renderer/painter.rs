use std::sync::Arc;
use std::time::Instant;

use ash::vk;
use rikka_colorimetry::BlendSpace;
use rikka_sync::Mutex;
use rikka_types::{Rect, Size, Transform};

use crate::draw::batcher::{Batch, Batcher};
use crate::draw::occlusion::Occlusion;
use crate::draw::primitive_pipeline::{PrimitivePipeline, Store};
use crate::draw::scene_pass::ScenePass;
use crate::fx::PostEffect;
use crate::fx::executor::{FxExecutor, PassInput, PassOutput};
use crate::fx::frame_target::FrameTarget;
use crate::renderer::Colors;
use crate::renderer::canvas::{Canvas, Plan};
use crate::renderer::copy_out::{CopyTarget, record_copy};
use crate::renderer::draw::RenderOutput;
use crate::renderer::history::{Targets, Viewing};
use crate::renderer::preparer::{Preparer, Shared};
use crate::renderer::sources::Sources;
use crate::renderer::work::FrameWork;
use crate::scene::{FrameBackdrops, Scene, SceneId, Viewer};
use crate::text::atlas::{AtlasKey, GlyphAtlas, SharedAtlas};
use crate::text::workers::GlyphWorkers;
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame::FrameTimeline;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::{
    ImageAccess, color_subresource_range, image_barrier, record_barriers,
};
use rikka_gpu::core::resource_table::TextureSlots;
use rikka_gpu::core::runtime::{GpuRuntime, Recycler};
use rikka_gpu::core::table_image::TableImage;
use rikka_gpu::error::Result;
use rikka_gpu::reclaim::Reclaimer;
use rikka_gpu::texture::TextureBook;

pub(crate) struct Painter {
    pub fx: FxExecutor,
    pub work: FrameWork,
    primitive: PrimitivePipeline,
    atlas: SharedAtlas,
    preparer: Preparer,
    shared: Shared,
    rects: FrameRects,
    backdrops: FrameBackdrops,
    occlusion: Occlusion,
    submission: Submission,
}

#[derive(Default)]
struct FrameRects {
    scene: Vec<Rect>,
    pass: Vec<Rect>,
    entry: Vec<Rect>,
    buffer: Vec<Rect>,
    copies: Vec<vk::BufferImageCopy>,
}

#[derive(Default)]
struct Submission {
    exit: Vec<vk::ImageMemoryBarrier2<'static>>,
    signals: Vec<vk::SemaphoreSubmitInfo<'static>>,
    scissors: Vec<Rect>,
}

struct Framing<'a> {
    composes: bool,
    pass: &'a [Rect],
    buffer: &'a [Rect],
    padding: f32,
    viewport: Rect,
    copy_out: Option<CopyRegions<'a>>,
}

#[derive(Clone, Copy)]
struct CopyRegions<'a> {
    buffer: vk::Buffer,
    regions: &'a [vk::BufferImageCopy],
}

#[derive(Clone, Copy)]
struct SceneDraw<'a> {
    scene: &'a Scene,
    viewer: Viewer,
    batch: &'a Batch<'a>,
    instance_set: vk::DescriptorSet,
    table_set: vk::DescriptorSet,
}

struct FilterPass {
    input: PassInput,
    region: Rect,
}

pub(crate) struct PaintState<'a> {
    pub targets: &'a mut Targets,
    pub sources: &'a mut Sources,
    pub colors: &'a Colors,
}

pub(crate) struct Sharing {
    pub book: Arc<TextureBook>,
    pub slots: TextureSlots,
    pub owner: Reclaimer,
    pub timeline: Arc<FrameTimeline>,
}

impl Painter {
    pub(crate) fn new(
        device: Arc<RenderDevice>,
        table: vk::DescriptorSetLayout,
        sharing: Sharing,
    ) -> Result<Self> {
        let atlas = Arc::new(Mutex::new(GlyphAtlas::new(device.clone(), sharing.slots)));
        let workers = Arc::new(GlyphWorkers::new(
            Arc::clone(&atlas),
            Arc::clone(&sharing.timeline),
        ));
        let shared = Shared {
            book: sharing.book,
            atlas: Arc::clone(&atlas),
            workers,
            owner: sharing.owner,
            timeline: sharing.timeline,
            routing: Default::default(),
        };
        Ok(Self {
            fx: FxExecutor::new(device.clone(), table),
            work: FrameWork::default(),
            primitive: PrimitivePipeline::new(device, table)?,
            atlas,
            preparer: Preparer::new(shared.clone()),
            shared,
            rects: FrameRects::default(),
            backdrops: FrameBackdrops::default(),
            occlusion: Occlusion::default(),
            submission: Submission::default(),
        })
    }

    pub(crate) fn shared(&self) -> Shared {
        self.shared.clone()
    }

    pub(crate) fn release_glyphs(&mut self, keys: &[AtlasKey]) {
        self.atlas.lock().release(keys);
    }

    pub(crate) fn trim(&mut self, recycler: &mut Recycler<'_>, now: Instant) {
        self.fx.trim(recycler, now);
        self.primitive.trim(recycler);
    }

    pub(crate) fn forget_scene(&mut self, scene: SceneId) -> Option<Store> {
        self.primitive.forget(scene)
    }

    pub(crate) fn frame(
        &mut self,
        gpu: &mut GpuRuntime,
        state: PaintState<'_>,
        scene: &mut Scene,
        canvas: &Canvas,
        copy: Option<CopyTarget<'_>>,
    ) -> Result<RenderOutput> {
        let PaintState {
            targets,
            sources,
            colors,
        } = state;
        let plan = canvas.plan;
        let viewport = plan.viewport();
        let scene_size = Size::new(viewport.width(), viewport.height());
        let now = Instant::now();

        self.fx.begin_frame(Targets::scene_format(&plan.space), now);
        self.preparer.prepare(scene)?;
        let retire_at = gpu.next_value();
        if sources
            .textures
            .grow_minified(&mut gpu.bind_point(), retire_at)
        {
            self.preparer.prepare(scene)?;
        }
        let viewing = Viewing {
            scene: scene.id(),
            viewer: plan.viewer,
        };
        if targets.prepare(gpu, &plan, viewing) && !scene.is_immediate() {
            scene.damage_whole_for(plan.viewer);
        }

        let composes = plan.composes(scene);
        if !composes {
            targets.rest_scene_color(gpu, plan.target);
        } else if targets.ensure_scene_color(
            gpu,
            plan.target,
            Targets::scene_format(&plan.space),
        )? && !scene.is_immediate()
        {
            scene.damage_whole_for(plan.viewer);
        }

        let mut rects = std::mem::take(&mut self.rects);
        let mut backdrops = std::mem::take(&mut self.backdrops);
        scene.take_damage_into(
            plan.viewer,
            viewport,
            &mut rects.scene,
            &mut backdrops,
            &|plan, cache| self.fx.stale_corners(plan, cache),
        );
        self.fx.keep_alive(scene.kept_blur_keys());
        scene.restage_settling();
        let padding = FxExecutor::chain_spread(scene.effects(), viewport.size);
        rects.entry.clear();
        rects.entry.extend(
            rects
                .scene
                .iter()
                .filter_map(|rect| rect.inflate(padding).intersection(&viewport)),
        );
        targets.damage_for_buffer(
            plan.target,
            &rects.entry,
            plan.buffer,
            viewport,
            &mut rects.buffer,
        );
        let immediate = scene.is_immediate();
        if composes {
            match immediate {
                true => {
                    rects.pass.clear();
                    rects.pass.extend_from_slice(&rects.scene);
                }
                false => {
                    targets.damage_for_scene(plan.target, &rects.scene, viewport, &mut rects.pass)
                }
            }
        }
        let widened = match (composes, immediate) {
            (_, true) => None,
            (true, false) => Some(&mut rects.pass),
            (false, false) => Some(&mut rects.buffer),
        };
        if let Some(widened) = widened
            && *widened != rects.scene
        {
            scene.expand_buffer_for_backdrops(
                plan.viewer,
                widened,
                viewport,
                &mut backdrops,
                &|plan, cache| self.fx.stale_corners(plan, cache),
            );
        }
        if let Some(copy) = &copy {
            copy.regions(
                rects
                    .buffer
                    .iter()
                    .map(|rect| plan.transform.map_rect(*rect, scene_size)),
                canvas.texels,
                &mut rects.copies,
            );
        }
        let framing = Framing {
            composes,
            pass: match (composes, immediate) {
                (true, _) => &rects.pass,
                (false, true) => &rects.scene,
                (false, false) => &rects.buffer,
            },
            buffer: &rects.buffer,
            padding,
            viewport,
            copy_out: copy.map(|copy| CopyRegions {
                buffer: copy.buffer,
                regions: &rects.copies,
            }),
        };

        self.backdrops = backdrops;
        let painted = match self.record(
            gpu,
            PaintState {
                targets,
                sources,
                colors,
            },
            scene,
            canvas,
            &framing,
            now,
        ) {
            Ok(recorded) => {
                self.atlas.lock().commit();
                self.work.add(recorded);
                tracing::trace!(
                    target = ?plan.target,
                    composes,
                    painted = framing.pass.len(),
                    nodes = recorded.walked,
                    emitted = recorded.emitted,
                    reused = recorded.reused,
                    restructured = recorded.restructured,
                    upload = recorded.uploaded_bytes,
                    area = framing
                        .pass
                        .iter()
                        .map(|rect| rect.width() * rect.height())
                        .sum::<f32>(),
                    "frame"
                );
                let damage = rects
                    .entry
                    .iter()
                    .map(|rect| plan.transform.map_rect(*rect, scene_size))
                    .collect();
                targets.record(plan.target, plan.buffer, &rects.entry, composes);
                Ok(RenderOutput {
                    damage,
                    needs_full_redraw: false,
                })
            }
            Err(error) => {
                self.abandon(gpu, sources, scene);
                Err(error)
            }
        };
        self.rects = rects;
        painted
    }

    fn abandon(&mut self, gpu: &mut GpuRuntime, sources: &mut Sources, scene: &mut Scene) {
        let frame_value = gpu.next_value();
        self.fx.abandon(gpu.bind_point().resources, frame_value);
        gpu.abandon_frame();
        sources.revert();
        self.primitive.revert_uploads();
        let mut atlas = self.atlas.lock();
        atlas.revert();
        if let Some(cache) = scene.cache.as_mut() {
            cache.forget(&mut atlas);
        }
        drop(atlas);
        if !scene.is_immediate() {
            scene.damage_all();
        }
    }

    fn record(
        &mut self,
        gpu: &mut GpuRuntime,
        state: PaintState<'_>,
        scene: &mut Scene,
        canvas: &Canvas,
        damage: &Framing<'_>,
        now: Instant,
    ) -> Result<FrameWork> {
        let PaintState {
            targets,
            sources,
            colors,
        } = state;
        let plan = canvas.plan;
        let mut ctx = gpu.begin_frame()?;

        ctx.begin_pass("rikka.sources");
        let video = sources.record(&mut ctx)?;
        ctx.end_pass();
        ctx.waits_on(video.waits);
        ctx.signals_after_setup(video.signals);

        let (work, curves) = {
            let mut atlas = self.atlas.lock();
            let curves = atlas.record(&mut ctx)?;
            let work = scene
                .cache
                .as_mut()
                .and_then(|cache| cache.prepared.take())
                .map(|prepared| prepared.settle(&mut atlas))
                .unwrap_or_default();
            (work, curves)
        };
        let scene: &Scene = scene;
        let batch = Batcher::batch(scene).expect("scene is prepared before recording");
        let uploaded = self.primitive.upload(
            &mut ctx,
            &batch.upload,
            curves,
            canvas.plan.viewer,
            (colors, &plan.space),
            now,
        )?;
        sources
            .textures
            .take_over(&mut ctx, batch.textured.iter().copied());
        let draw = SceneDraw {
            scene,
            viewer: plan.viewer,
            batch: &batch,
            instance_set: uploaded.set,
            table_set: ctx.resources.set(),
        };
        let filtered = match damage.composes {
            false => None,
            true => self.record_composed(&mut ctx, targets, draw, damage, &plan)?,
        };

        ctx.begin_canvas();
        let range = color_subresource_range();
        record_barriers(
            ctx.device,
            ctx.cmd,
            &[image_barrier(
                canvas.image,
                canvas.entry(),
                ImageAccess::COLOR_ATTACHMENT,
                canvas.acquire,
                range,
            )],
        );
        if !damage.composes {
            ctx.begin_pass("rikka.scene");
            let frame_target = FrameTarget {
                image: canvas.image,
                view: canvas.view,
                format: canvas.format,
                texture: canvas.texels,
                turn: plan.transform,
                scene: damage.viewport,
                space: plan.space,
            };
            self.draw_scene(&mut ctx, draw, frame_target, damage.pass)?;
            ctx.end_pass();
        } else if let Some(filtered) = filtered {
            ctx.begin_pass("rikka.composite");
            composite(
                &mut self.fx,
                &mut ctx,
                canvas,
                filtered,
                damage,
                &mut self.submission.scissors,
            )?;
            ctx.end_pass();
        }

        let Submission { exit, signals, .. } = &mut self.submission;
        exit.clear();
        exit.push(image_barrier(
            canvas.image,
            ImageAccess::COLOR_ATTACHMENT,
            canvas.after,
            canvas.release,
            range,
        ));
        sources.textures.extend_release_barriers(exit);
        self.fx.settle(&ctx);
        record_barriers(ctx.device, ctx.cmd, exit);
        if let Some(copy) = damage.copy_out {
            ctx.begin_pass("rikka.copy_out");
            record_copy(ctx.device, ctx.cmd, canvas.image, copy.buffer, copy.regions);
            ctx.end_pass();
        }
        self.fx.end_frame(&mut ctx);

        signals.clear();
        signals.extend(canvas.signal);
        let submitted = gpu.submit(canvas.wait.as_slice(), signals)?;
        sources.commit(gpu, submitted);

        Ok(FrameWork {
            frames: 1,
            walked: work.walked as u64,
            emitted: work.emitted as u64,
            reused: work.reused as u64,
            restructured: u64::from(work.restructured),
            uploaded_instances: uploaded.instances as u64,
            uploaded_bytes: uploaded.bytes as u64,
        })
    }

    fn record_composed(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        targets: &mut Targets,
        draw: SceneDraw<'_>,
        damage: &Framing<'_>,
        plan: &Plan,
    ) -> Result<Option<FilterPass>> {
        let scene_color = targets
            .state(plan.target)
            .scene_color_mut()
            .expect("composed frame has a scene image");
        if !damage.pass.is_empty() {
            ctx.begin_pass("rikka.scene");
            scene_color
                .image
                .barrier_to(ctx.cmd, ImageAccess::COLOR_ATTACHMENT);
            let image = &scene_color.image;
            let frame_target = FrameTarget {
                image: image.handle(),
                view: image.view(),
                format: image.format(),
                texture: image.extent(),
                turn: Transform::Normal,
                scene: damage.viewport,
                space: plan.space,
            };
            self.draw_scene(ctx, draw, frame_target, damage.pass)?;
            ctx.end_pass();
        }
        scene_color
            .image
            .barrier_to(ctx.cmd, ImageAccess::FRAGMENT_SAMPLED);
        ctx.begin_pass("rikka.effects");
        let filtered = filter(&mut self.fx, ctx, scene_color, draw.scene.effects(), damage)?;
        ctx.end_pass();
        Ok(filtered)
    }

    fn draw_scene(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        draw: SceneDraw<'_>,
        frame_target: FrameTarget,
        damage: &[Rect],
    ) -> Result<()> {
        ScenePass {
            primitive: &mut self.primitive,
            fx: &mut self.fx,
            occlusion: &mut self.occlusion,
            backdrops: &self.backdrops,
            scene: draw.scene,
            viewer: draw.viewer,
            batch: draw.batch,
            instance_set: draw.instance_set,
            table_set: draw.table_set,
            frame_target,
        }
        .record(ctx, damage)
    }
}

fn filter(
    fx: &mut FxExecutor,
    ctx: &mut GpuFrameCtx<'_>,
    scene_color: &TableImage,
    effects: &[Arc<dyn PostEffect>],
    damage: &Framing<'_>,
) -> Result<Option<FilterPass>> {
    let Some(covered) = damage.buffer.iter().copied().reduce(|a, b| a.union(&b)) else {
        return Ok(None);
    };
    let region = match effects.is_empty() {
        true => covered,
        false => covered
            .inflate(damage.padding)
            .round_out()
            .intersection(&damage.viewport)
            .unwrap_or(damage.viewport),
    };
    let input = fx.run_chain(ctx, effects, PassInput::from_table(scene_color, region))?;
    Ok(Some(FilterPass { input, region }))
}

fn composite(
    fx: &mut FxExecutor,
    ctx: &mut GpuFrameCtx<'_>,
    canvas: &Canvas,
    filtered: FilterPass,
    damage: &Framing<'_>,
    scissors: &mut Vec<Rect>,
) -> Result<()> {
    let plan = canvas.plan;
    let scene_size = Size::new(damage.viewport.width(), damage.viewport.height());
    let output = PassOutput {
        view: canvas.view,
        format: canvas.format,
        region: plan.transform.map_rect(filtered.region, scene_size),
    };
    scissors.clear();
    scissors.extend(
        damage
            .buffer
            .iter()
            .map(|rect| plan.transform.map_rect(*rect, scene_size)),
    );
    match plan.space {
        BlendSpace::Sdr => fx.composite(
            ctx,
            filtered.input,
            output,
            plan.transform.inverse(),
            scissors,
        ),
        BlendSpace::Hdr(space) => fx.encode_pq(
            ctx,
            filtered.input,
            output,
            (plan.transform.inverse(), space),
            scissors,
        ),
    }
}
