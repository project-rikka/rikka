use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use std::thread;

use rikka_physics_core::{Error, Executor, ExecutorExt, SerialExecutor, Stage, WorkerPool};

fn pools() -> Vec<Box<dyn Executor>> {
    let mut executors: Vec<Box<dyn Executor>> = vec![Box::new(SerialExecutor)];
    for workers in [1, 2, 3, 8] {
        executors.push(Box::new(
            WorkerPool::builder().workers(workers).build().unwrap(),
        ));
    }
    executors
}

#[test]
fn blocks_run_once_in_stage_order() {
    for executor in pools() {
        let stages: Vec<_> = (0..24)
            .map(|stage| Stage::new("stage", [0, 1, 2, 7, 64, 3][stage % 6]))
            .collect();
        let counts: Vec<Vec<AtomicU32>> = stages
            .iter()
            .map(|stage| (0..stage.blocks).map(|_| AtomicU32::new(0)).collect())
            .collect();
        let workers = executor.workers() as u32;
        for _ in 0..50 {
            executor.run(&stages, &|block| {
                assert!(block.worker < workers);
                for earlier in &counts[..block.stage as usize] {
                    assert!(
                        earlier
                            .iter()
                            .all(|count| count.load(Ordering::Relaxed) > 0)
                    );
                }
                counts[block.stage as usize][block.index as usize].fetch_add(1, Ordering::Relaxed);
            });
            for count in counts.iter().flatten() {
                assert_eq!(count.swap(0, Ordering::Relaxed), 1);
            }
        }
    }
}

#[test]
fn task_panic_propagates_and_pool_survives() {
    assert!(matches!(
        WorkerPool::builder().workers(0).build(),
        Err(Error::NoWorkers)
    ));
    let pool = WorkerPool::builder().workers(4).build().unwrap();
    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        pool.for_each(100, |block| {
            if block.index == 57 {
                panic!("block 57 failed");
            }
        });
    }));
    let payload = result.unwrap_err();
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"block 57 failed"));
    let total = AtomicUsize::new(0);
    pool.for_each(100, |_| {
        total.fetch_add(1, Ordering::Relaxed);
    });
    assert_eq!(total.into_inner(), 100);
}

#[test]
fn nested_and_concurrent_runs_complete() {
    let pool = WorkerPool::builder().workers(3).build().unwrap();
    let nested = AtomicUsize::new(0);
    pool.for_each(16, |outer| {
        pool.for_each(8, |inner| {
            assert_eq!(inner.worker, outer.worker);
            nested.fetch_add(1, Ordering::Relaxed);
        });
    });
    assert_eq!(nested.into_inner(), 128);

    let concurrent = AtomicUsize::new(0);
    thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..100 {
                    pool.for_each(10, |_| {
                        concurrent.fetch_add(1, Ordering::Relaxed);
                    });
                }
            });
        }
    });
    assert_eq!(concurrent.into_inner(), 4000);
}

#[test]
fn parked_workers_wake_for_each_run() {
    let pool = WorkerPool::builder().workers(4).build().unwrap();
    let stages = [Stage::new("a", 9), Stage::new("b", 1), Stage::new("c", 5)];
    let total = AtomicUsize::new(0);
    for run in 0..3000u32 {
        if run.is_multiple_of(500) {
            thread::sleep(std::time::Duration::from_millis(2));
        }
        pool.run(&stages, &|block| {
            if block.stage == 2 && block.index == 0 && run.is_multiple_of(97) {
                thread::sleep(std::time::Duration::from_millis(1));
            }
            total.fetch_add(1, Ordering::Relaxed);
        });
    }
    assert_eq!(total.into_inner(), 3000 * 15);
}
