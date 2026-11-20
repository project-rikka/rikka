use rikka_gpu::Batch;
use rikka_physics_gpu::Pass;

use crate::error::Result;
use crate::fluid::Fluid;
use crate::stage::Frame;
use crate::substance::Substance;

const SPRAY_PER_PARTICLE: u32 = 8;

impl Fluid {
    pub(crate) fn render_state(&mut self, batch: &mut Batch, spray_count: u32) -> Result<Frame> {
        let slots = self.clock.slots;
        let motion = self.clock.moved.then_some(&self.particles.motion);
        self.stage.look.record(
            batch,
            &self.stage.kernels,
            self.particles.state(),
            motion,
            slots,
        );
        let spray = self.stage.spray_ring.acquire(&self.stage.compute)?;
        self.foam
            .pack(batch, &self.stage.kernels, &spray, spray_count);
        self.stage.render_record(batch, slots, spray, spray_count)
    }

    pub(crate) fn spray_bound(&self, measured: u32, candidates: u32) -> u32 {
        self.foam
            .capacity
            .min(measured.saturating_add(candidates.saturating_mul(SPRAY_PER_PARTICLE)))
    }

    pub(crate) fn measure(&self, batch: &mut Batch) {
        batch.fill(&self.stats, 0);
        let state = self.particles.state();
        let bonds = self.substances.iter().any(Substance::bonds);
        let (bond_counts, partners) = if bonds {
            (&state.bond_counts, &state.partners)
        } else {
            (&self.particles.placeholder, &self.particles.placeholder)
        };
        let (offsets, triangles) = match &self.stage.bodies.buffers {
            Some(buffers) => (buffers.tri_offsets(), buffers.triangle_count),
            None => (&self.stats, 0),
        };
        Pass::direct(&self.stage.kernels.stats, self.clock.slots.max(1), 128)
            .read(&state.positions)
            .read(&state.velocities)
            .read(&state.phases)
            .read(bond_counts)
            .read(partners)
            .read(&self.foam.counters)
            .read(offsets)
            .write(&self.stats)
            .uint(self.clock.slots)
            .uint(self.particles.stride)
            .uint(triangles)
            .uint(u32::from(bonds))
            .run(batch);
    }
}
