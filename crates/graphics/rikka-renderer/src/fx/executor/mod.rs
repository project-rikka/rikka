mod backdrops;
mod capture;
mod chain;
mod effect_output;
mod filter;
mod pass_input;
mod record;

use std::sync::{Arc, LazyLock};
use std::time::Instant;

use ash::vk;
use rikka_types::{Point, Rect};

use crate::fx::anchor::Anchor;
use crate::fx::cache::{BackdropCache, BackdropStore};
use crate::fx::effect::{BackdropPass, BackdropShader};
use crate::fx::frame_target::FrameTarget;
use crate::fx::pipelines::FxPipelines;
use crate::fx::pool::{FX_FORMAT, FxTargetPool, HDR_FX_FORMAT};
use crate::fx::recipe::{Plan, StaleRegion};
use crate::fx::{blur, refract};
use rikka_gpu::core::device::RenderDevice;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::record_barriers;
use rikka_gpu::core::pipeline::{Blend, CANVAS_FORMATS};
use rikka_gpu::core::resource_table::ResourceTable;
use rikka_gpu::core::runtime::Recycler;
use rikka_gpu::error::Result;

use filter::SampledSlot;
use record::FxPushConstants;

pub(crate) use effect_output::{Capture, EffectOutput};
pub(crate) use pass_input::{PassInput, PassOutput};

static COMPOSITE_SHADER: LazyLock<BackdropShader> =
    LazyLock::new(|| BackdropShader::builtin(shaders::COMPOSITE_FRAG, "rikka.fx.composite"));

static OUTPUT_PQ_SHADER: LazyLock<BackdropShader> =
    LazyLock::new(|| BackdropShader::builtin(shaders::OUTPUT_PQ_FRAG, "rikka.fx.output_pq"));

fn family() -> Vec<(BackdropShader, vk::Format, Blend)> {
    let composite = COMPOSITE_SHADER.clone();
    let pq = OUTPUT_PQ_SHADER.clone();
    let mut family = Vec::new();
    for format in [FX_FORMAT, HDR_FX_FORMAT] {
        for shader in blur::shaders().into_iter().chain([refract::shader()]) {
            family.push((shader, format, Blend::Opaque));
        }
    }
    for format in CANVAS_FORMATS.into_iter().chain([FX_FORMAT, HDR_FX_FORMAT]) {
        for blend in [Blend::Opaque, Blend::Mix] {
            family.push((composite.clone(), format, blend));
        }
    }
    for format in CANVAS_FORMATS {
        family.push((pq.clone(), format, Blend::Opaque));
    }
    family
}

#[derive(Default)]
pub(super) struct FxScratch {
    pub(super) stale_rects: Vec<Rect>,
    pub(super) stale_regions: Vec<StaleRegion>,
    pub(super) writes: Vec<Rect>,
    pub(super) turned: Vec<Rect>,
}

pub(crate) struct FxExecutor {
    device: Arc<RenderDevice>,
    pipelines: Arc<FxPipelines>,
    pool: FxTargetPool,
    cache: BackdropStore,
    retained_keys: Vec<u64>,
    alive_keys: Vec<u64>,
    pending_barriers: Vec<vk::ImageMemoryBarrier2<'static>>,
    sampled: Option<SampledSlot>,
    format: vk::Format,
    frame_now: Instant,
    scratch: FxScratch,
}

impl FxExecutor {
    pub(crate) fn new(device: Arc<RenderDevice>, table_layout: vk::DescriptorSetLayout) -> Self {
        let push_size = size_of::<FxPushConstants>() as u32;
        Self {
            pipelines: FxPipelines::start(device.clone(), table_layout, push_size, family()),
            device,
            pool: FxTargetPool::new(),
            cache: BackdropStore::default(),
            retained_keys: Vec::new(),
            alive_keys: Vec::new(),
            pending_barriers: Vec::new(),
            sampled: None,
            format: FX_FORMAT,
            frame_now: Instant::now(),
            scratch: FxScratch::default(),
        }
    }

    pub(crate) fn begin_frame(&mut self, format: vk::Format, now: Instant) {
        self.format = format;
        self.frame_now = now;
    }

    pub(crate) fn defer_barrier(&mut self, barrier: Option<vk::ImageMemoryBarrier2<'static>>) {
        let Some(barrier) = barrier else {
            return;
        };
        match self
            .pending_barriers
            .iter_mut()
            .find(|stale| stale.image == barrier.image)
        {
            Some(stale) => {
                stale.dst_stage_mask = barrier.dst_stage_mask;
                stale.dst_access_mask = barrier.dst_access_mask;
                stale.new_layout = barrier.new_layout;
            }
            None => self.pending_barriers.push(barrier),
        }
    }

    pub(crate) fn settle(&mut self, ctx: &GpuFrameCtx<'_>) {
        record_barriers(&self.device, ctx.cmd, &self.pending_barriers);
        self.pending_barriers.clear();
    }

    fn unsettled<T>(&mut self, result: Result<T>) -> Result<T> {
        if result.is_err() {
            self.pending_barriers.clear();
        }
        result
    }

    pub(crate) fn run_effect(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        plan: &Plan,
        frame_target: &FrameTarget,
        cache: Option<&BackdropCache>,
    ) -> Result<Option<EffectOutput>> {
        let result = self.filter(ctx, plan, frame_target, cache);
        self.unsettled(result)
    }

    fn run_pooled(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        plan: &Plan,
        input: PassInput,
        want: Rect,
        last: &BackdropPass,
    ) -> Result<EffectOutput> {
        let anchor = Anchor::new(input.region.origin, last.scale);
        let whole = [StaleRegion::whole(input.region)];
        let (current, current_pool) = self.record_chain(
            ctx,
            plan,
            plan.passes.len(),
            chain::ChainInput {
                input,
                want,
                anchor,
            },
            &whole,
        )?;
        let out_index = current_pool.expect("non-empty pass list writes a pooled target");
        self.readable(out_index);
        Ok(EffectOutput {
            pool_index: Some(out_index),
            tex: self.pool.tex(out_index),
            anchor,
            texture_size: self.pool.image(out_index).extent(),
            bounds: current.texels(),
        })
    }

    pub(crate) fn release_output(&mut self, output: EffectOutput) {
        if let Some(index) = output.pool_index {
            self.pool.release(index);
        }
    }

    pub(crate) fn abandon(&mut self, resources: &mut ResourceTable, frame_value: u64) {
        self.pending_barriers.clear();
        if let Some(slot) = self.sampled.take() {
            resources.release(slot.tex, frame_value);
        }
        self.pool.abandon(frame_value);
    }

    pub(crate) fn end_frame(&mut self, ctx: &mut GpuFrameCtx<'_>) {
        self.settle(ctx);
        if let Some(slot) = self.sampled.take() {
            ctx.resources.release(slot.tex, ctx.frame_value);
        }
        let now = self.frame_now;
        self.cache.refresh(&self.retained_keys, now);
        self.cache.refresh(&self.alive_keys, now);
        self.pool.end_frame(ctx, now);
        self.cache.end_frame(ctx, now);
    }

    pub(crate) fn trim(&mut self, recycler: &mut Recycler<'_>, now: Instant) {
        self.pool.trim(recycler, now);
        self.cache.trim(recycler, now);
    }
    fn pool_input(&self, index: usize, region: Rect) -> PassInput {
        PassInput {
            tex: self.pool.tex(index),
            texture_size: self.pool.image(index).extent(),
            region,
            origin: Point::ZERO,
            scale: 1.0,
        }
    }

    fn level_input(&self, index: usize, texels: Rect, anchor: Anchor) -> PassInput {
        let scale = anchor.scale().recip();
        let origin = anchor.origin();
        PassInput {
            region: Rect::new(
                texels.min_x() * scale + origin.x,
                texels.min_y() * scale + origin.y,
                texels.width() * scale,
                texels.height() * scale,
            ),
            origin,
            scale,
            ..self.pool_input(index, Rect::ZERO)
        }
    }
}

impl Drop for FxExecutor {
    fn drop(&mut self) {
        self.pipelines.close();
    }
}

pub(super) fn coalesce_rects(rects: &mut Vec<Rect>) {
    rects.retain(|rect| !rect.is_empty());
    let mut index = 0;
    while index < rects.len() {
        let mut merged = false;
        let mut other = index + 1;
        while other < rects.len() {
            if merges_without_waste(rects[index], rects[other]) {
                rects[index] = rects[index].union(&rects[other]);
                rects.swap_remove(other);
                merged = true;
            } else {
                other += 1;
            }
        }
        if !merged {
            index += 1;
        }
    }
}

fn merges_without_waste(a: Rect, b: Rect) -> bool {
    if a.contains_rect(&b) || b.contains_rect(&a) {
        return true;
    }
    if a.min_y() == b.min_y() && a.max_y() == b.max_y() {
        return a.min_x() <= b.max_x() && b.min_x() <= a.max_x();
    }
    if a.min_x() == b.min_x() && a.max_x() == b.max_x() {
        return a.min_y() <= b.max_y() && b.min_y() <= a.max_y();
    }
    false
}
