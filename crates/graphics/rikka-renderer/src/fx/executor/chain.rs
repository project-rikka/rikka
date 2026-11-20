use std::sync::Arc;

use ash::vk;
use rikka_types::{Rect, Size};

use crate::fx::anchor::Anchor;
use crate::fx::effect::PostEffect;
use crate::fx::recipe::{Plan, Recipe, StaleRegion};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::ImageAccess;
use rikka_gpu::core::pipeline::Blend;
use rikka_gpu::error::Result;

use super::pass_input::{PassInput, PassOutput};
use super::record::{Composite, Recorded};
use super::{FxExecutor, coalesce_rects};

pub(super) struct ChainInput {
    pub(super) input: PassInput,
    pub(super) want: Rect,
    pub(super) anchor: Anchor,
}

impl FxExecutor {
    pub(super) fn record_chain(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        plan: &Plan,
        upto: usize,
        source: ChainInput,
        stale: &[StaleRegion],
    ) -> Result<(PassInput, Option<usize>)> {
        let ChainInput {
            input,
            want,
            anchor,
        } = source;
        let mut current = input;
        let mut written: Option<usize> = None;
        let mut writes = std::mem::take(&mut self.scratch.writes);
        let result = (|| {
            for (index, pass) in plan.passes[..upto].iter().enumerate() {
                let step = anchor.on(pass.scale);
                let target_scene = pass.target(index == 0, input.region, want);
                let region = step.rect(target_scene);
                let out_index = self.pool.acquire(
                    ctx,
                    region.max_x() as u32,
                    region.max_y() as u32,
                    self.format,
                )?;
                if let Some(prev) = written {
                    let barrier = self.pool.transition(prev, ImageAccess::FRAGMENT_SAMPLED);
                    self.defer_barrier(barrier);
                }
                let barrier = self
                    .pool
                    .transition(out_index, ImageAccess::COLOR_ATTACHMENT);
                self.defer_barrier(barrier);

                writes.clear();
                writes.extend(stale.iter().filter_map(|piece| {
                    step.rect(plan.writes(piece, index)).intersection(&region)
                }));
                coalesce_rects(&mut writes);

                let output = PassOutput {
                    view: self.pool.image(out_index).view(),
                    format: self.format,
                    region,
                };
                self.record_pass(
                    ctx,
                    Recorded {
                        label: pass.shader.label(),
                        shader: &pass.shader,
                        params: pass.params_clamped_to(current.uv_bounds()),
                        input: current,
                        src_scene: Some(step.scene_rect(region)),
                        output,
                        load_op: vk::AttachmentLoadOp::DONT_CARE,
                        composite: Composite::Over(Blend::Opaque),
                        scissors: &writes,
                    },
                )?;
                if let Some(previous) = written.replace(out_index) {
                    self.pool.release(previous);
                }
                current = self.level_input(out_index, region, step);
            }
            Ok((current, written))
        })();
        self.scratch.writes = writes;
        result
    }

    pub(super) fn readable(&mut self, index: usize) {
        let barrier = self.pool.transition(index, ImageAccess::FRAGMENT_SAMPLED);
        self.defer_barrier(barrier);
    }

    pub(crate) fn chain_spread(effects: &[Arc<dyn PostEffect>], region: Size) -> f32 {
        effects
            .iter()
            .map(|effect| effect.spread(region).ceil())
            .sum()
    }

    pub(crate) fn run_chain(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        effects: &[Arc<dyn PostEffect>],
        input: PassInput,
    ) -> Result<PassInput> {
        let result = self.chain(ctx, effects, input);
        self.unsettled(result)
    }

    fn chain(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        effects: &[Arc<dyn PostEffect>],
        mut input: PassInput,
    ) -> Result<PassInput> {
        let mut active: Option<usize> = None;
        for effect in effects {
            let plan = Recipe::whole(effect.as_ref(), input.region).plan(input.region);
            let Some(last) = plan.passes.last() else {
                continue;
            };
            let out = self.run_pooled(ctx, &plan, input, input.region, last)?;
            let index = out
                .pool_index
                .expect("effect without a cache uses a pooled target");
            if let Some(previous) = active.replace(index) {
                self.pool.release(previous);
            }
            input = self.pool_input(index, out.bounds);
        }
        Ok(input)
    }
}
