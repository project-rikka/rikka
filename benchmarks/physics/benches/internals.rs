use std::sync::LazyLock;

use divan::Bencher;
use divan::counter::ItemsCount;
use physics_bench::{
    EXECUTOR_EMPTY, EXECUTOR_STAGES, EXECUTOR_UNEVEN, Internal, RADIX_SORT, RAGDOLL_RAIN, RAIN,
    SLEEP, Size,
};

static SIZE: LazyLock<Size> = LazyLock::new(Size::from_args);

fn main() {
    divan::main();
}

fn worker_counts() -> Vec<usize> {
    SIZE.worker_counts()
}

fn run(bencher: Bencher, internal: &Internal, workers: usize) {
    let timed = internal.steps(*SIZE) - 1;
    bencher
        .counter(ItemsCount::new(timed))
        .with_inputs(|| {
            let mut workload = internal.create(workers).expect("workload should build");
            workload.step();
            workload
        })
        .bench_local_values(|mut workload| {
            for _ in 0..timed {
                workload.step();
            }
            workload
        });
}

#[divan::bench(args = worker_counts(), sample_count = 10, sample_size = 1)]
fn executor_empty(bencher: Bencher, workers: usize) {
    run(bencher, &EXECUTOR_EMPTY, workers);
}

#[divan::bench(args = worker_counts(), sample_count = 10, sample_size = 1)]
fn executor_stages(bencher: Bencher, workers: usize) {
    run(bencher, &EXECUTOR_STAGES, workers);
}

#[divan::bench(args = worker_counts(), sample_count = 10, sample_size = 1)]
fn executor_uneven(bencher: Bencher, workers: usize) {
    run(bencher, &EXECUTOR_UNEVEN, workers);
}

#[divan::bench(args = worker_counts(), sample_count = 10, sample_size = 1)]
fn radix_sort(bencher: Bencher, workers: usize) {
    run(bencher, &RADIX_SORT, workers);
}

#[divan::bench(args = worker_counts(), sample_count = 5, sample_size = 1)]
fn rain(bencher: Bencher, workers: usize) {
    run(bencher, &RAIN, workers);
}

#[divan::bench(args = worker_counts(), sample_count = 5, sample_size = 1)]
fn ragdoll_rain(bencher: Bencher, workers: usize) {
    run(bencher, &RAGDOLL_RAIN, workers);
}

#[divan::bench(args = worker_counts(), sample_count = 5, sample_size = 1)]
fn sleep(bencher: Bencher, workers: usize) {
    run(bencher, &SLEEP, workers);
}
