use glam::Vec3;
use rikka_gpu::Compute;
use rikka_gpu::{Batch, ComputeBuffer};
use rikka_physics_gpu::{Pass, Scan};

use crate::error::Result;
use crate::gpu::bodies::BodyBuffers;
use crate::gpu::kernels::Kernels;
use crate::gpu::particles::Particles;

const VEC4: u64 = 16;
const WORD: u64 = 4;

pub(crate) struct Foam {
    pub(crate) positions: [ComputeBuffer; 2],
    pub(crate) velocities: [ComputeBuffer; 2],
    pub(crate) meta: [ComputeBuffer; 2],
    pub(crate) current: usize,
    alive: ComputeBuffer,
    pub(crate) counters: ComputeBuffer,
    pub(crate) capacity: u32,
    scan: Scan,
}

pub(crate) struct FoamInputs<'a> {
    pub(crate) settings: &'a ComputeBuffer,
    pub(crate) particles: &'a Particles,
    pub(crate) starts: &'a ComputeBuffer,
    pub(crate) bodies: Option<&'a BodyBuffers>,
    pub(crate) count: u32,
    pub(crate) resident: u32,
    pub(crate) dt: f32,
    pub(crate) gravity: Vec3,
    pub(crate) seed: u32,
}

impl Foam {
    pub(crate) fn new(compute: &Compute, capacity: u32) -> Result<Foam> {
        let wide = u64::from(capacity.max(1));
        Ok(Foam {
            positions: [compute.buffer(wide * VEC4)?, compute.buffer(wide * VEC4)?],
            velocities: [compute.buffer(wide * VEC4)?, compute.buffer(wide * VEC4)?],
            meta: [compute.buffer(wide * WORD)?, compute.buffer(wide * WORD)?],
            current: 0,
            alive: compute.buffer((wide + 1) * WORD)?,
            counters: compute.buffer(4 * WORD)?,
            capacity,
            scan: Scan::new(compute)?,
        })
    }

    pub(crate) fn step(&mut self, batch: &mut Batch, kernels: &Kernels, fluid: &FoamInputs<'_>) {
        if self.capacity == 0 {
            return;
        }
        let state = fluid.particles.state();
        let current = self.current;
        let next = 1 - current;
        let resident = fluid.resident.min(self.capacity);
        if fluid.count > 0 {
            self.scan
                .run(batch, &fluid.particles.foam_counts, fluid.count + 1);
        }
        Pass::direct(&kernels.foam_advect, resident + 1, 128)
            .read(fluid.settings)
            .read(&state.positions)
            .read(&state.velocities)
            .read(&state.phases)
            .read(fluid.starts)
            .write(&self.positions[current])
            .write(&self.velocities[current])
            .write(&self.meta[current])
            .write(&self.alive)
            .read(&self.counters)
            .uint(resident)
            .float(fluid.dt)
            .float(fluid.gravity.x)
            .float(fluid.gravity.y)
            .float(fluid.gravity.z)
            .run(batch);
        if let Some(bodies) = fluid.bodies {
            Pass::direct(&kernels.spray_collide, resident, 128)
                .read(fluid.settings)
                .write(&self.positions[current])
                .write(&self.velocities[current])
                .read(&self.alive)
                .read(bodies.skin_to())
                .read(&bodies.triangles)
                .read(&bodies.tri_starts)
                .read(&bodies.tri_items)
                .uint(resident)
                .float(fluid.dt)
                .run(batch);
        }
        self.scan.run(batch, &self.alive, resident + 1);
        Pass::direct(&kernels.foam_compact, resident.max(1), 128)
            .read(&self.alive)
            .read(&self.positions[current])
            .read(&self.velocities[current])
            .read(&self.meta[current])
            .write(&self.positions[next])
            .write(&self.velocities[next])
            .write(&self.meta[next])
            .write(&self.counters)
            .uint(resident)
            .run(batch);
        Pass::direct(&kernels.foam_spawn, fluid.count.max(1), 128)
            .read(fluid.settings)
            .read(&state.positions)
            .read(&state.velocities)
            .read(&state.phases)
            .read(&fluid.particles.foam_counts)
            .write(&self.positions[next])
            .write(&self.velocities[next])
            .write(&self.meta[next])
            .write(&self.counters)
            .uint(fluid.count)
            .uint(self.capacity)
            .uint(fluid.seed)
            .run(batch);
        self.current = next;
    }

    pub(crate) fn pack(
        &self,
        batch: &mut Batch,
        kernels: &Kernels,
        into: &ComputeBuffer,
        count: u32,
    ) {
        Pass::direct(&kernels.spray_pack, count, 128)
            .read(&self.positions[self.current])
            .read(&self.velocities[self.current])
            .read(&self.meta[self.current])
            .read(&self.counters)
            .write(into)
            .uint(count)
            .run(batch);
    }

    pub(crate) fn persistent(&self) -> [(&ComputeBuffer, u64); 3] {
        [
            (&self.positions[self.current], VEC4),
            (&self.velocities[self.current], VEC4),
            (&self.meta[self.current], WORD),
        ]
    }
}
