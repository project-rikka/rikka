use rikka_gpu::Compute;
use rikka_gpu::{Batch, ComputeBuffer};
use rikka_physics_gpu::Pass;

use crate::error::Result;
use crate::gpu::kernels::Kernels;
use crate::gpu::particles::State;
use crate::gpu::settings::Grid;
use crate::gpu::sorter::{Keys, Sorter, Sorting};

pub(crate) const RECORD_POSITION: u64 = 16;
pub(crate) const RECORD_VELOCITY: u64 = 8;

pub(crate) struct Look {
    pub(crate) positions: ComputeBuffer,
    pub(crate) velocities: ComputeBuffer,
    sorted: ComputeBuffer,
    sorter: Sorter,
}

pub(crate) struct SurfaceInputs<'a> {
    pub(crate) kernels: &'a Kernels,
    pub(crate) settings: &'a ComputeBuffer,
    pub(crate) grid: Grid,
    pub(crate) into: &'a ComputeBuffer,
    pub(crate) count: u32,
}

impl Look {
    pub(crate) fn new(compute: &Compute, capacity: u32, widest: Grid) -> Result<Look> {
        let wide = u64::from(capacity);
        Ok(Look {
            positions: compute.buffer(wide * RECORD_POSITION)?,
            velocities: compute.buffer(wide * RECORD_VELOCITY)?,
            sorted: compute.buffer(wide * RECORD_POSITION)?,
            sorter: Sorter::new(compute, capacity, widest)?,
        })
    }

    pub(crate) fn record(
        &self,
        batch: &mut Batch,
        kernels: &Kernels,
        state: &State,
        motion: Option<&ComputeBuffer>,
        count: u32,
    ) {
        Pass::direct(&kernels.pack, count, 128)
            .read(&state.positions)
            .read(&state.velocities)
            .read(&state.phases)
            .write(&self.positions)
            .write(&self.velocities)
            .read(motion.unwrap_or(&state.velocities))
            .uint(count)
            .uint(u32::from(motion.is_some()))
            .run(batch);
    }

    pub(crate) fn shape(&self, batch: &mut Batch, surface_inputs: &SurfaceInputs<'_>) {
        if surface_inputs.count == 0 {
            return;
        }
        Sorting {
            sorter: &self.sorter,
            kernels: surface_inputs.kernels,
            settings: surface_inputs.settings,
            keys: Keys::Display {
                records: &self.positions,
                sorted: &self.sorted,
            },
            grid: surface_inputs.grid,
            count: surface_inputs.count,
        }
        .run(batch);
        Pass::direct(
            &surface_inputs.kernels.look_shape,
            surface_inputs.count,
            128,
        )
        .read(surface_inputs.settings)
        .read(&self.sorted)
        .read(&self.sorter.perm)
        .read(&self.sorter.starts)
        .read(&self.velocities)
        .write(surface_inputs.into)
        .uint(surface_inputs.count)
        .run(batch);
    }
}
