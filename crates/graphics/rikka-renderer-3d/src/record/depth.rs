use ash::vk;

use crate::build::BuiltFrame;
use crate::error::Result;
use crate::frame_data::FrameAddresses;
use crate::gpu::GpuPassPush;
use crate::graph::{Attachment, FrameGraph, Load};
use crate::material::MaterialPass;
use crate::pipeline::DrawState;
use crate::recipes::{self, ViewFormats};
use crate::record::Record;
use crate::record::cull::draws_read;
use crate::record::stage::{SceneImages, Stage};
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

pub(super) const CLEAR_DEPTH: f32 = 0.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Phase {
    Early { late: bool },
    Late,
}

impl Phase {
    fn clears(self) -> bool {
        matches!(self, Self::Early { .. })
    }

    fn is_last(self) -> bool {
        matches!(self, Self::Late | Self::Early { late: false })
    }

    fn feeds_pyramid(self) -> bool {
        self == Self::Early { late: true }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Early { .. } => "rikka.three.prepass",
            Self::Late => "rikka.three.late_prepass",
        }
    }
}

pub(super) fn clear_depth() -> Load {
    Load::Clear(vk::ClearValue {
        depth_stencil: vk::ClearDepthStencilValue {
            depth: CLEAR_DEPTH,
            stencil: 0,
        },
    })
}

impl Record<'_> {
    pub(super) fn declare_prepass<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &SceneImages,
        phase: Phase,
    ) {
        let view = stage.view;
        let load = match phase.clears() {
            true => clear_depth(),
            false => Load::Keep,
        };
        let mut depth = Attachment::new(scene.depth, load);
        if view.multisampled && phase.feeds_pyramid() {
            depth = depth.resolve(scene.depth_resolved, view.depth_resolve);
        }
        let mut pass = graph.render(phase.name()).depth(depth);
        if let Some(target) = scene.identity_target {
            let load = match phase.clears() {
                true => Load::Clear(vk::ClearValue {
                    color: vk::ClearColorValue { uint32: [0; 4] },
                }),
                false => Load::Keep,
            };
            let mut identity = Attachment::new(target, load);
            if let (true, true, Some(resolved)) = (
                view.multisampled && view.identity_samples,
                phase.is_last(),
                scene.identity,
            ) {
                identity = identity
                    .transient()
                    .resolve(resolved, vk::ResolveModeFlags::SAMPLE_ZERO);
            }
            pass = pass.color(identity);
        }
        for (buffer, usage) in draws_read() {
            pass = pass.buffer(buffer, usage);
        }
        let built = stage.built;
        let formats = view.formats;
        pass.record(move |record, ctx, scope| {
            record.depth_prepass(ctx, built, scope.addresses, formats, phase)
        });
    }

    fn depth_prepass(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        built: &BuiltFrame,
        addresses: &FrameAddresses,
        view: ViewFormats,
        phase: Phase,
    ) -> Result<()> {
        for (index, bucket) in built.buckets.iter().enumerate() {
            if !bucket.pass.in_prepass() || bucket.hull {
                continue;
            }
            let at = match phase {
                Phase::Early { .. } => Some(index as u32),
                Phase::Late => built.layout.late(index as u32),
            };
            let Some(at) = at else {
                continue;
            };
            let masked = bucket.pass == MaterialPass::Masked;
            let recipe = recipes::prepass(
                &self.mesh.builtin,
                &bucket.program,
                bucket.state,
                masked,
                view,
            );
            let pipeline = self.pipeline(&recipe)?;
            self.bind(
                ctx,
                pipeline,
                DrawState::depth_tested(bucket.state.cull, vk::CompareOp::GREATER, true),
            );
            self.push(
                ctx,
                &GpuPassPush {
                    frame: addresses.refs,
                    slot: 0,
                    flags: 0,
                },
            );
            self.draw_range(
                ctx,
                addresses,
                at,
                built.bucket_table[at as usize],
                bucket.indices,
            );
        }
        Ok(())
    }
}
