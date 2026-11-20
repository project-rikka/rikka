use rikka_physics_core::{RadixSorter, WorkerPool};

use super::{Workload, WorkloadResult};
use super::{mix, pool};

const ITEMS: usize = 1 << 20;

struct RadixSort {
    pool: WorkerPool,
    sorter: RadixSorter,
    input: Vec<(u64, u32)>,
    items: Vec<(u64, u32)>,
    scratch: Vec<(u64, u32)>,
}

pub(crate) fn create(threads: usize) -> WorkloadResult {
    let input: Vec<_> = (0..ITEMS as u32)
        .map(|payload| (mix(u64::from(payload)) >> 20, payload))
        .collect();
    Ok(Box::new(RadixSort {
        pool: pool(threads)?,
        sorter: RadixSorter::new(),
        items: input.clone(),
        scratch: input.clone(),
        input,
    }))
}

impl Workload for RadixSort {
    fn step(&mut self) {
        self.items.copy_from_slice(&self.input);
        self.sorter
            .par_sort(&self.pool, &mut self.items, &mut self.scratch, |item| {
                item.0
            });
    }

    fn checksum(&self) -> u64 {
        self.items
            .iter()
            .step_by(997)
            .fold(0, |sum, &(key, payload)| {
                mix(sum ^ key ^ u64::from(payload))
            })
    }
}
