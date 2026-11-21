use std::thread;

use rikka_parallel::{Parallel, Threads};

const NAME: &str = "rikka-parallel-test";

fn value(index: usize) -> u64 {
    (index as u64)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .rotate_left(17)
}

#[test]
fn results_follow_input_order_for_every_thread_count() {
    let len = 1_003;
    let expected: Vec<u64> = (0..len).map(value).collect();
    for threads in [1, 2, 3, 8, 64] {
        let parallel = Parallel::new(NAME).threads(Threads::new(threads)).grain(7);
        assert_eq!(
            parallel.collect(len, value),
            expected,
            "collect, {threads} threads"
        );
        let mut filled = vec![0; len];
        parallel.fill(&mut filled, |index, slot| *slot = value(index));
        assert_eq!(filled, expected, "fill, {threads} threads");
        let chunked = parallel
            .map_chunks(len, |range| range.map(value).collect::<Vec<_>>())
            .concat();
        assert_eq!(chunked, expected, "map_chunks, {threads} threads");
        let mapped = parallel.map(0..len, value);
        assert_eq!(mapped, expected, "map, {threads} threads");
        let stateful = parallel.map_with(
            (0..len).collect::<Vec<_>>(),
            Vec::new,
            |seen: &mut Vec<usize>, index| {
                seen.push(index);
                value(index)
            },
        );
        assert_eq!(stateful, expected, "map_with, {threads} threads");
    }
}

#[test]
fn nested_calls_stay_on_the_worker_thread() {
    let outer = Parallel::new(NAME).threads(Threads::new(4));
    let inner = Parallel::new("rikka-parallel-inner").threads(Threads::new(4));
    let checks = outer.map(0..4, |_| {
        let here = thread::current().id();
        inner
            .collect(256, |_| thread::current().id())
            .into_iter()
            .all(|id| id == here)
    });
    assert!(checks.into_iter().all(|same| same));
}
