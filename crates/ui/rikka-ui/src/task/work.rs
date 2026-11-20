use std::sync::OnceLock;

use tokio::runtime::{Builder, Runtime};

static WORK: OnceLock<Runtime> = OnceLock::new();

const MIN_WORKERS: usize = 2;

const MAX_WORKERS: usize = 4;

pub fn work_runtime() -> &'static Runtime {
    WORK.get_or_init(|| {
        Builder::new_multi_thread()
            .worker_threads(workers())
            .thread_name("rikka-work")
            .enable_all()
            .build()
            .expect("failed to start work runtime")
    })
}

fn workers() -> usize {
    std::thread::available_parallelism()
        .map_or(MIN_WORKERS, |cores| cores.get() / 2)
        .clamp(MIN_WORKERS, MAX_WORKERS)
}
