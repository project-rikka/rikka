use ash::vk;
use rikka_types::{Point, Rect, Transform};

use crate::fx::cache::BackdropCache;
use crate::fx::frame_target::FrameTarget;
use crate::fx::recipe::Plan;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;
use rikka_gpu::core::image::ImageAccess;
use rikka_gpu::core::pipeline::Blend;
use rikka_gpu::error::Result;

use super::effect_output::{EffectOutput, cached_output};
use super::pass_input::{PassInput, PassOutput};
use super::record::{Composite, Recorded, mapped};
use super::{COMPOSITE_SHADER, FxExecutor, coalesce_rects};

struct SavedRegion {
    input: PassInput,
    pooled: Option<usize>,
}

#[derive(Clone, Copy)]
pub(super) struct SampledSlot {
    frame: u64,
    view: vk::ImageView,
    pub(super) tex: u32,
}

impl FxExecutor {
    pub(super) fn filter(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        plan: &Plan,
        frame_target: &FrameTarget,
        cache: Option<&BackdropCache>,
    ) -> Result<Option<EffectOutput>> {
        let Some((last, leading)) = plan.passes.split_last() else {
            return Ok(None);
        };
        let want = plan.region;
        let Some(cache) = cache else {
            let saved = self.save_region(ctx, frame_target, want, want)?;
            let output = self.run_pooled(ctx, plan, saved.input, want, last);
            self.restore_region(&saved, frame_target);
            return output.map(Some);
        };

        let cached = self.cache.take(cache.key);
        let now = self.frame_now;
        let mut into = self.cache.fit(
            ctx,
            cached,
            want,
            (last.scale, plan.signature),
            self.format,
            now,
        )?;

        let reshaped = plan.reshaped(into.rounded);
        into.rounded = plan.rounding.map(|rounding| rounding.shape);
        let mut stale_rects = std::mem::take(&mut self.scratch.stale_rects);
        stale_rects.clear();
        want.subtract(&into.valid, &mut stale_rects);
        for rect in cache.stale.iter().chain(reshaped.iter()) {
            if let Some(inside) = rect
                .round_out()
                .intersection(&want)
                .and_then(|rect| rect.intersection(&into.valid))
            {
                stale_rects.push(inside);
            }
        }
        coalesce_rects(&mut stale_rects);
        if stale_rects.is_empty() {
            self.scratch.stale_rects = stale_rects;
            into.valid = want;
            let result = cached_output(&into, want);
            self.cache.put(cache.key, into, now);
            return Ok(Some(result));
        }

        let mut stale = std::mem::take(&mut self.scratch.stale_regions);
        stale.clear();
        stale.extend(stale_rects.iter().map(|rect| plan.stale_region(*rect)));
        self.scratch.stale_rects = stale_rects;

        let reads = stale
            .iter()
            .map(|piece| plan.lift_of(piece))
            .reduce(|all, rect| all.union(&rect))
            .and_then(|reads| reads.round_out().intersection(&want))
            .unwrap_or(want);
        let saved = match self.save_region(ctx, frame_target, want, reads) {
            Ok(saved) => saved,
            Err(error) => {
                self.scratch.stale_regions = stale;
                into.table.retire(&mut ctx.recycler());
                return Err(error);
            }
        };

        let anchor = into.anchor;
        let chained = self.record_chain(
            ctx,
            plan,
            leading.len(),
            super::chain::ChainInput {
                input: saved.input,
                want,
                anchor,
            },
            &stale,
        );
        if !leading.is_empty() {
            self.restore_region(&saved, frame_target);
        }
        let (current, mut current_pool) = match chained {
            Ok(chained) => chained,
            Err(error) => {
                if leading.is_empty() {
                    self.restore_region(&saved, frame_target);
                }
                self.scratch.stale_regions = stale;
                into.table.retire(&mut ctx.recycler());
                return Err(error);
            }
        };

        let target_scene = last.target(leading.is_empty(), saved.input.region, want);
        let out_region = anchor.rect(target_scene);
        if let Some(index) = current_pool {
            let barrier = self.pool.transition(index, ImageAccess::FRAGMENT_SAMPLED);
            self.defer_barrier(barrier);
        }
        self.defer_barrier(into.table.image.transition(ImageAccess::COLOR_ATTACHMENT));

        let mut writes = std::mem::take(&mut self.scratch.writes);
        writes.clear();
        writes.extend(
            stale
                .iter()
                .filter_map(|piece| anchor.rect(piece.rect).intersection(&out_region)),
        );
        coalesce_rects(&mut writes);
        self.scratch.stale_regions = stale;

        let output = PassOutput {
            view: into.table.image.view(),
            format: into.table.image.format(),
            region: out_region,
        };
        let recorded = self.record_pass(
            ctx,
            Recorded {
                label: "rikka.fx.kept",
                shader: &last.shader,
                params: last.params_clamped_to(current.uv_bounds()),
                input: current,
                src_scene: Some(anchor.scene_rect(out_region)),
                output,
                load_op: vk::AttachmentLoadOp::LOAD,
                composite: Composite::Over(Blend::Opaque),
                scissors: &writes,
            },
        );
        self.scratch.writes = writes;
        if leading.is_empty() {
            self.restore_region(&saved, frame_target);
        }
        if let Err(error) = recorded {
            into.table.retire(&mut ctx.recycler());
            return Err(error);
        }
        self.defer_barrier(into.table.image.transition(ImageAccess::FRAGMENT_SAMPLED));
        if let Some(previous) = current_pool.take() {
            self.pool.release(previous);
        }

        into.valid = want;
        let result = cached_output(&into, want);
        self.cache.put(cache.key, into, now);
        Ok(Some(result))
    }

    fn save_region(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        frame_target: &FrameTarget,
        region: Rect,
        reads: Rect,
    ) -> Result<SavedRegion> {
        let source = self.read_from(ctx, frame_target)?;
        if frame_target.turn == Transform::Normal {
            return Ok(SavedRegion {
                input: PassInput { region, ..source },
                pooled: None,
            });
        }
        let upright = Rect::new(0.0, 0.0, reads.width(), reads.height());
        let into = self.pool.acquire(
            ctx,
            upright.width() as u32,
            upright.height() as u32,
            self.format,
        )?;
        let barrier = self.pool.transition(into, ImageAccess::COLOR_ATTACHMENT);
        self.defer_barrier(barrier);
        let recorded = self.record_pass(
            ctx,
            Recorded {
                label: "rikka.fx.upright",
                shader: &COMPOSITE_SHADER,
                params: mapped(frame_target.turn),
                input: PassInput {
                    region: frame_target.texels(reads),
                    ..source
                },
                src_scene: None,
                output: PassOutput {
                    view: self.pool.image(into).view(),
                    format: self.format,
                    region: upright,
                },
                load_op: vk::AttachmentLoadOp::DONT_CARE,
                composite: Composite::Over(Blend::Opaque),
                scissors: &[upright],
            },
        );
        if let Err(error) = recorded {
            self.pool.release(into);
            return Err(error);
        }
        self.readable(into);
        Ok(SavedRegion {
            input: PassInput {
                region: reads,
                origin: reads.origin,
                ..self.pool_input(into, Rect::ZERO)
            },
            pooled: Some(into),
        })
    }

    pub(super) fn read_from(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        frame_target: &FrameTarget,
    ) -> Result<PassInput> {
        let tex = match self.sampled {
            Some(slot) if slot.frame == ctx.frame_value && slot.view == frame_target.view => {
                slot.tex
            }
            _ => {
                if let Some(stale) = self.sampled.take() {
                    ctx.resources.release(stale.tex, ctx.frame_value);
                }
                let tex = ctx
                    .resources
                    .allocate(frame_target.view, ctx.samplers.linear_clamp())?;
                self.sampled = Some(SampledSlot {
                    frame: ctx.frame_value,
                    view: frame_target.view,
                    tex,
                });
                tex
            }
        };
        self.defer_barrier(Some(
            frame_target.barrier(ImageAccess::COLOR_ATTACHMENT, ImageAccess::FRAGMENT_SAMPLED),
        ));
        Ok(PassInput {
            tex,
            texture_size: frame_target.texture,
            region: Rect::ZERO,
            origin: Point::ZERO,
            scale: 1.0,
        })
    }

    pub(super) fn recycle(&mut self, frame_target: &FrameTarget) {
        self.defer_barrier(Some(
            frame_target.barrier(ImageAccess::FRAGMENT_SAMPLED, ImageAccess::COLOR_ATTACHMENT),
        ));
    }

    fn restore_region(&mut self, saved: &SavedRegion, frame_target: &FrameTarget) {
        if let Some(index) = saved.pooled {
            self.pool.release(index);
        }
        self.recycle(frame_target);
    }
}
