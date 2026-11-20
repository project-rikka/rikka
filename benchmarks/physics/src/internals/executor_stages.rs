use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

use rikka_physics_core::{Executor, Stage, WorkerPool};

use super::{Workload, WorkloadResult};
use super::{mix, pool};

const STAGES: usize = 64;
const BLOCKS: usize = 32;
const ROUNDS_PER_BLOCK: u64 = 64;

struct ExecutorStages {
    pool: WorkerPool,
    stages: Vec<Stage>,
    cells: [Vec<AtomicU64>; 2],
}

pub(crate) fn create(threads: usize) -> WorkloadResult {
    Ok(Box::new(ExecutorStages {
        pool: pool(threads)?,
        stages: vec![Stage::new("small", BLOCKS as u32); STAGES],
        cells: [0, 1].map(|side| {
            (0..BLOCKS as u64)
                .map(|cell| AtomicU64::new(cell + side))
                .collect()
        }),
    }))
}

impl Workload for ExecutorStages {
    fn step(&mut self) {
        let cells = &self.cells;
        self.pool.run(&self.stages, &|block| {
            let stage = block.stage as usize;
            let index = block.index as usize;
            let (read, write) = (&cells[stage % 2], &cells[(stage + 1) % 2]);
            let mut value = read[index].load(Relaxed) ^ read[(index + 1) % BLOCKS].load(Relaxed);
            for round in 0..ROUNDS_PER_BLOCK {
                value = mix(value + round);
            }
            write[index].store(value, Relaxed);
        });
    }

    fn checksum(&self) -> u64 {
        self.cells
            .iter()
            .flatten()
            .fold(0, |sum, cell| mix(sum ^ cell.load(Relaxed)))
    }
}
