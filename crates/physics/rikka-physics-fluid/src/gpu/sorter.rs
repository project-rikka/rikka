use rikka_gpu::Compute;
use rikka_gpu::{Batch, ComputeBuffer};
use rikka_physics_gpu::{Pass, Scan};

use crate::error::Result;
use crate::gpu::kernels::Kernels;
use crate::gpu::particles::State;
use crate::gpu::settings::Grid;

const WORD: u64 = 4;

pub(crate) struct Sorter {
    keys: ComputeBuffer,
    slots: ComputeBuffer,
    order: ComputeBuffer,
    pub(crate) perm: ComputeBuffer,
    pub(crate) inverse: ComputeBuffer,
    pub(crate) starts: ComputeBuffer,
    scan: Scan,
}

#[derive(Clone, Copy)]
pub(crate) enum Keys<'a> {
    Simulation {
        state: &'a State,
        dt: f32,
    },
    Display {
        records: &'a ComputeBuffer,
        sorted: &'a ComputeBuffer,
    },
}

pub(crate) struct Sorting<'a> {
    pub(crate) sorter: &'a Sorter,
    pub(crate) kernels: &'a Kernels,
    pub(crate) settings: &'a ComputeBuffer,
    pub(crate) keys: Keys<'a>,
    pub(crate) grid: Grid,
    pub(crate) count: u32,
}

impl Sorter {
    pub(crate) fn new(compute: &Compute, capacity: u32, widest: Grid) -> Result<Sorter> {
        let wide = u64::from(capacity) * WORD;
        Ok(Sorter {
            keys: compute.buffer(wide)?,
            slots: compute.buffer(wide)?,
            order: compute.buffer(wide)?,
            perm: compute.buffer(wide)?,
            inverse: compute.buffer(wide)?,
            starts: compute.buffer((u64::from(widest.bins()) + 2) * WORD)?,
            scan: Scan::new(compute)?,
        })
    }
}

impl Sorting<'_> {
    pub(crate) fn run(&self, batch: &mut Batch) {
        let mut stage = 0;
        while self.stage(batch, stage) {
            stage += 1;
        }
    }

    pub(crate) fn stage(&self, batch: &mut Batch, stage: usize) -> bool {
        let sorter = self.sorter;
        let bins = self.grid.bins();
        match stage {
            0 => {
                batch.fill_range(&sorter.starts, 0, (u64::from(bins) + 2) * WORD, 0);
                self.count_keys(batch);
            }
            1 => sorter.scan.run(batch, &sorter.starts, bins + 2),
            2 => Pass::direct(&self.kernels.sort_place, self.count, 128)
                .read(&sorter.keys)
                .read(&sorter.slots)
                .read(&sorter.starts)
                .write(&sorter.order)
                .uint(self.count)
                .run(batch),
            3 => {
                let pass = Pass::direct(&self.kernels.sort_rank, self.count, 128)
                    .read(&sorter.keys)
                    .read(&sorter.slots)
                    .read(&sorter.starts)
                    .read(&sorter.order)
                    .write(&sorter.perm)
                    .write(&sorter.inverse);
                let pass = match self.keys {
                    Keys::Simulation { .. } => pass.read(&sorter.keys).read(&sorter.keys),
                    Keys::Display { records, sorted } => pass.read(records).write(sorted),
                };
                pass.uint(self.count)
                    .uint(bins)
                    .uint(u32::from(matches!(self.keys, Keys::Display { .. })))
                    .run(batch);
            }
            _ => return false,
        }
        true
    }

    fn count_keys(&self, batch: &mut Batch) {
        let sorter = self.sorter;
        match self.keys {
            Keys::Simulation { state, dt } => {
                Pass::direct(&self.kernels.sort_count, self.count, 128)
                    .read(self.settings)
                    .read(&state.positions)
                    .read(&state.velocities)
                    .read(&state.phases)
                    .write(&sorter.keys)
                    .write(&sorter.slots)
                    .write(&sorter.starts)
                    .uint(self.count)
                    .float(dt)
                    .run(batch)
            }
            Keys::Display { records, .. } => {
                Pass::direct(&self.kernels.look_count, self.count, 128)
                    .read(self.settings)
                    .read(records)
                    .write(&sorter.keys)
                    .write(&sorter.slots)
                    .write(&sorter.starts)
                    .uint(self.count)
                    .run(batch)
            }
        }
    }
}
