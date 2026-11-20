mod reader;
mod writer;

use std::sync::atomic::AtomicU64;

pub use reader::{Recording, Stamps};
pub use writer::Probe;

pub const PROBE_ENV: &str = "UI_BENCH_PROBE_FD";

const CAPACITY: usize = 1 << 16;

#[repr(C)]
struct Shared {
    frames: AtomicU64,
    stamps: [AtomicU64; CAPACITY],
}

const SHARED_SIZE: usize = size_of::<Shared>();

#[must_use]
pub fn monotonic_ns() -> u64 {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    now.tv_sec as u64 * 1_000_000_000 + now.tv_nsec as u64
}
