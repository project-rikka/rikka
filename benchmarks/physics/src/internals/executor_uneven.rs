use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use rikka_physics_core::{ExecutorExt, WorkerPool};

use super::{Workload, WorkloadResult};
use super::{mix, pool};

const BLOCKS: usize = 256;
const LIGHT_ROUNDS: u64 = 200;
const HEAVY_ROUNDS: u64 = 20_000;
const HEAVY_EVERY: usize = 37;

struct ExecutorUneven {
    pool: WorkerPool,
    results: Vec<AtomicU64>,
}

pub(crate) fn create(threads: usize) -> WorkloadResult {
    Ok(Box::new(ExecutorUneven {
        pool: pool(threads)?,
        results: (0..BLOCKS).map(|_| AtomicU64::new(0)).collect(),
    }))
}

fn rounds(block: usize) -> u64 {
    if block.is_multiple_of(HEAVY_EVERY) {
        HEAVY_ROUNDS
    } else {
        LIGHT_ROUNDS * (1 + (mix(block as u64) % 8))
    }
}

impl Workload for ExecutorUneven {
    fn step(&mut self) {
        let results = &self.results;
        self.pool.for_each(BLOCKS as u32, |block| {
            let index = block.index as usize;
            let mut value = results[index].load(Relaxed);
            for round in 0..rounds(index) {
                value = mix(value ^ round);
            }
            results[index].store(value, Relaxed);
        });
    }

    fn checksum(&self) -> u64 {
        self.results
            .iter()
            .fold(0, |sum, result| mix(sum ^ result.load(Relaxed)))
    }
}
