use rikka_physics_core::{RadixSorter, SerialExecutor, WorkerPool};

fn random(len: usize, seed: u64, mask: u64) -> Vec<(u64, u32)> {
    let mut state = seed;
    (0..len as u32)
        .map(|payload| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state & mask, payload)
        })
        .collect()
}

#[test]
fn sort_is_stable_for_all_key_widths_and_worker_counts() {
    let pools: Vec<_> = [2, 3, 8]
        .into_iter()
        .map(|workers| WorkerPool::builder().workers(workers).build().unwrap())
        .collect();
    let mut sorter = RadixSorter::new();
    for (len, mask) in [
        (10, 7),
        (5000, 0xff),
        (100_000, u64::MAX),
        (70_001, 0x3ff),
        (40_000, 0xffff_ffff),
        (40_000, 0),
        (40_000, 0xffff_f800),
    ] {
        let items = random(len, 12345, mask);
        let mut wide_reference = items.clone();
        wide_reference.sort_by_key(|item| item.0);
        let mut narrow_reference = items.clone();
        narrow_reference.sort_by_key(|item| item.0 as u32);
        let mut scratch = vec![(0, 0); len];

        let mut serial = items.clone();
        sorter.sort(&mut serial, &mut scratch, |item| item.0);
        assert_eq!(serial, wide_reference);
        let mut serial = items.clone();
        sorter.par_sort(&SerialExecutor, &mut serial, &mut scratch, |item| item.0);
        assert_eq!(serial, wide_reference);
        for pool in &pools {
            let mut sorted = items.clone();
            sorter.par_sort(pool, &mut sorted, &mut scratch, |item| item.0);
            assert_eq!(sorted, wide_reference);
            let mut narrow = items.clone();
            sorter.par_sort(pool, &mut narrow, &mut scratch, |item| item.0 as u32);
            assert_eq!(narrow, narrow_reference);
        }
    }
}
