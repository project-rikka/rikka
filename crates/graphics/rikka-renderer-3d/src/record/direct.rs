use ash::vk;

use crate::build::{Bucket, BuiltFrame, DirectDraw};
use crate::error::Result;
use crate::frame_data::FrameAddresses;
use crate::gpu::GpuPassPush;
use crate::pipeline::{DrawState, Recipe};
use crate::recipes::{self, ViewFormats};
use crate::record::{PASS_SEE_THROUGH, Record, surface_formats};
use crate::state::Renderer3dState;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

#[derive(Clone, Copy)]
pub(super) enum DirectPass {
    Shade(ViewFormats),
    CharacterDepth,
    SeeThrough(ViewFormats),
    Surface { layers: bool, masked: bool },
    Occluder,
    Outline,
}

impl DirectPass {
    fn recipe(self, mesh: &Renderer3dState, bucket: &Bucket) -> Recipe {
        match self {
            Self::Shade(view) => recipes::shade(
                &mesh.builtin,
                &bucket.program,
                bucket.hull,
                bucket.state,
                bucket.features,
                view,
            ),
            Self::CharacterDepth => {
                recipes::character_depth(&mesh.builtin, &bucket.program, bucket.state)
            }
            Self::SeeThrough(view) => recipes::see_through(
                &mesh.builtin,
                &bucket.program,
                bucket.state,
                bucket.features,
                view,
            ),
            Self::Surface { layers, masked } => recipes::surfaces(
                &mesh.look_state.surface_vert,
                &bucket.program,
                bucket.state,
                surface_formats(layers),
                layers,
                masked,
            ),
            Self::Occluder => recipes::occluder(&mesh.builtin, &bucket.program, bucket.state),
            Self::Outline => recipes::outline_prepass(&mesh.builtin, &bucket.program, bucket.state),
        }
    }

    fn draw_state(self, bucket: &Bucket) -> DrawState {
        let cull = bucket.state.cull;
        match self {
            Self::Shade(_) => {
                DrawState::from_material(bucket.state, vk::CompareOp::GREATER_OR_EQUAL)
            }
            Self::CharacterDepth | Self::Surface { .. } | Self::Occluder => {
                DrawState::depth_tested(cull, vk::CompareOp::GREATER, true)
            }
            Self::Outline => DrawState::depth_tested(cull, vk::CompareOp::GREATER_OR_EQUAL, true),
            Self::SeeThrough(_) => DrawState::depth_tested(cull, vk::CompareOp::ALWAYS, false),
        }
    }

    fn flags(self) -> u32 {
        match self {
            Self::SeeThrough(_) => PASS_SEE_THROUGH,
            Self::Shade(_)
            | Self::CharacterDepth
            | Self::Surface { .. }
            | Self::Occluder
            | Self::Outline => 0,
        }
    }

    fn frame(self, addresses: &FrameAddresses) -> u64 {
        match self {
            Self::Occluder | Self::Outline => addresses.overlay_refs,
            Self::Shade(_) | Self::CharacterDepth | Self::SeeThrough(_) | Self::Surface { .. } => {
                addresses.refs
            }
        }
    }
}

impl Record<'_> {
    pub(super) fn bind_direct(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        addresses: &FrameAddresses,
        bucket: &Bucket,
        pass: DirectPass,
    ) -> Result<()> {
        let pipeline = self.pipeline(&pass.recipe(self.mesh, bucket))?;
        self.bind_pass(ctx, addresses, bucket, pass, pipeline);
        Ok(())
    }

    pub(super) fn draw_direct<'d>(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        built: &BuiltFrame,
        addresses: &FrameAddresses,
        draws: impl IntoIterator<Item = &'d DirectDraw>,
        pass: DirectPass,
    ) -> Result<()> {
        let mut bound: Option<(u32, vk::Pipeline)> = None;
        for draw in draws {
            let bucket = &built.buckets[draw.bucket as usize];
            let pipeline = match bound {
                Some((at, pipeline)) if at == draw.bucket => pipeline,
                _ => self.pipeline(&pass.recipe(self.mesh, bucket))?,
            };
            bound = Some((draw.bucket, pipeline));
            self.bind_pass(ctx, addresses, bucket, pass, pipeline);
            self.draw_indexed(
                ctx,
                draw.index_count,
                draw.first_index,
                draw.vertex_offset,
                draw.instance,
                bucket.indices,
            );
        }
        Ok(())
    }

    fn bind_pass(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        addresses: &FrameAddresses,
        bucket: &Bucket,
        pass: DirectPass,
        pipeline: vk::Pipeline,
    ) {
        self.bind(ctx, pipeline, pass.draw_state(bucket));
        self.push(
            ctx,
            &GpuPassPush {
                frame: pass.frame(addresses),
                slot: 0,
                flags: pass.flags(),
            },
        );
    }
}
