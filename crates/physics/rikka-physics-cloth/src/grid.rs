use rikka_gpu::Compute;
use rikka_gpu::{Batch, ComputeBuffer};
use rikka_physics_gpu::{MAX_SCANNED, Scan};

use crate::error::Result;

const CELLS: u64 = 196_608;
const WORD: u64 = 4;
const SPANNED_CELLS: u64 = 16;
const SPHERE: u64 = 16;
const LARGE_OBSTACLES: u64 = 4096;
const _: () = assert!(CELLS < MAX_SCANNED as u64);

pub(crate) struct Grid {
    pub(crate) cells: ComputeBuffer,
    pub(crate) items: ComputeBuffer,
    pub(crate) spheres: ComputeBuffer,
    pub(crate) large: ComputeBuffer,
}

impl Grid {
    pub(crate) fn new(compute: &Compute) -> Result<Grid> {
        Ok(Grid {
            cells: compute.buffer((2 * CELLS + 2) * WORD)?,
            items: compute.buffer(4096)?,
            spheres: compute.buffer(4096 * SPHERE)?,
            large: compute.buffer((LARGE_OBSTACLES + 1) * WORD)?,
        })
    }

    pub(crate) fn reserve(&mut self, compute: &Compute, obstacles: u64) -> Result<()> {
        let entries = (obstacles * SPANNED_CELLS).next_power_of_two();
        if entries * WORD > self.items.size() {
            self.items = compute.buffer(entries * WORD)?;
            self.spheres = compute.buffer(entries * SPHERE)?;
        }
        Ok(())
    }

    pub(crate) fn clear(&self, batch: &mut Batch) {
        batch.fill_range(&self.cells, 0, (CELLS + 1) * WORD, 0);
        batch.fill_range(&self.large, 0, WORD, 0);
    }

    pub(crate) fn scan(&self, batch: &mut Batch, scan: &Scan) {
        scan.run(batch, &self.cells, (CELLS + 1) as u32);
        batch.copy_buffer(
            &self.cells,
            0,
            &self.cells,
            (CELLS + 1) * WORD,
            CELLS * WORD,
        );
    }
}
