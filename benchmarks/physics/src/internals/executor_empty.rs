use std::hint::black_box;

use rikka_physics_core::{Executor, Stage, WorkerPool};

use super::pool;
use super::{Workload, WorkloadResult};

const BLOCKS: u32 = 16;

struct ExecutorEmpty {
    pool: WorkerPool,
    steps: u64,
}

pub(crate) fn create(threads: usize) -> WorkloadResult {
    Ok(Box::new(ExecutorEmpty {
        pool: pool(threads)?,
        steps: 0,
    }))
}

impl Workload for ExecutorEmpty {
    fn step(&mut self) {
        self.pool.run(&[Stage::new("empty", BLOCKS)], &|block| {
            black_box(block);
        });
        self.steps += 1;
    }

    fn checksum(&self) -> u64 {
        self.steps
    }
}
