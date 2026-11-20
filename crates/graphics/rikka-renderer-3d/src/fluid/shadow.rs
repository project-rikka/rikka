use ash::vk;

use crate::error::Result;
use crate::fluid::plan::FluidFrame;
use crate::frame_data::FrameAddresses;
use crate::material::Cull;
use crate::pipeline::DrawState;
use crate::record::Record;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

impl Record<'_> {
    pub(crate) fn fluid_shadow(
        &mut self,
        ctx: &GpuFrameCtx<'_>,
        fluid: &FluidFrame,
        addresses: &FrameAddresses,
        shadow: u32,
    ) -> Result<()> {
        let recipe = self.mesh.look_state.fluid.shadow_recipe();
        let pipeline = self.pipeline(&recipe)?;
        self.bind(
            ctx,
            pipeline,
            DrawState::depth_tested(Cull::None, vk::CompareOp::GREATER, true),
        );
        self.fluid_splats(ctx, fluid, addresses, shadow + 1);
        Ok(())
    }
}
