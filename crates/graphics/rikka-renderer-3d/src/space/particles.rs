use ash::vk;

use crate::error::Result;
use crate::frame_data::FrameAddresses;
use crate::gpu::GpuPassPush;
use crate::look::Look;
use crate::look::gpu::GpuSpace;
use crate::material::Cull;
use crate::pipeline::{DrawState, Recipe};
use crate::recipes::ViewFormats;
use crate::record::Record;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const CORNERS: u32 = 6;

pub(crate) fn particles_drawn(look: &Look) -> bool {
    GpuSpace::particle_count(look.particles.amount) > 0
}

impl Record<'_> {
    pub(super) fn space_particles_recipe(&self, view: ViewFormats) -> Recipe {
        self.mesh.look_state.space.particles_recipe(view)
    }

    pub(crate) fn particles(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        look: &Look,
        addresses: &FrameAddresses,
        view: ViewFormats,
    ) -> Result<()> {
        let count = GpuSpace::particle_count(look.particles.amount);
        if count == 0 {
            return Ok(());
        }
        let recipe = self.space_particles_recipe(view);
        let pipeline = self.pipeline(&recipe)?;
        self.bind(
            ctx,
            pipeline,
            DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER_OR_EQUAL, false),
        );
        self.push(
            ctx,
            &GpuPassPush {
                frame: addresses.refs,
                slot: 0,
                flags: 0,
            },
        );
        unsafe {
            ctx.device.handle().cmd_draw(ctx.cmd, CORNERS, count, 0, 0);
        }
        Ok(())
    }
}
