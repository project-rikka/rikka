mod executor_empty;
mod executor_stages;
mod executor_uneven;
mod radix_sort;
mod ragdoll_rain;
mod rain;
mod rigid;
mod sleep;

use std::time::Duration;

use rikka_physics_core::WorkerPool;

use crate::error::Result;
use crate::size::Size;
pub trait Workload {
    fn step(&mut self);

    fn checksum(&self) -> u64;

    fn enable_profile(&mut self) {}

    fn profile(&self) -> Vec<(&'static str, Duration, u32)> {
        Vec::new()
    }
}

pub(crate) type WorkloadResult = Result<Box<dyn Workload>>;

#[derive(Clone, Copy)]
pub struct Internal {
    pub name: &'static str,
    steps: u32,
    create: fn(usize) -> WorkloadResult,
}

impl Internal {
    const fn new(name: &'static str, steps: u32, create: fn(usize) -> WorkloadResult) -> Self {
        Self {
            name,
            steps,
            create,
        }
    }

    pub fn steps(&self, size: Size) -> u32 {
        size.steps(self.steps)
    }

    pub fn create(&self, workers: usize) -> Result<Box<dyn Workload>> {
        (self.create)(workers)
    }
}

impl std::fmt::Debug for Internal {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Internal")
            .field("name", &self.name)
            .field("steps", &self.steps)
            .finish()
    }
}

pub const EXECUTOR_EMPTY: Internal =
    Internal::new("executor_empty", 20_000, executor_empty::create);
pub const EXECUTOR_STAGES: Internal =
    Internal::new("executor_stages", 2_000, executor_stages::create);
pub const EXECUTOR_UNEVEN: Internal =
    Internal::new("executor_uneven", 200, executor_uneven::create);
pub const RADIX_SORT: Internal = Internal::new("radix_sort", 50, radix_sort::create);
pub const RAIN: Internal = Internal::new("rain", 400, rain::create);
pub const RAGDOLL_RAIN: Internal = Internal::new("ragdoll_rain", 400, ragdoll_rain::create);
pub const SLEEP: Internal = Internal::new("sleep", 300, sleep::create);

pub const INTERNALS: &[Internal] = &[
    EXECUTOR_EMPTY,
    EXECUTOR_STAGES,
    EXECUTOR_UNEVEN,
    RADIX_SORT,
    RAIN,
    RAGDOLL_RAIN,
    SLEEP,
];

fn pool(threads: usize) -> rikka_physics_core::Result<WorkerPool> {
    WorkerPool::builder()
        .workers(threads)
        .thread_name(|worker| format!("rikka-physics-bench-{worker}"))
        .build()
}

fn mix(value: u64) -> u64 {
    let value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}
