use rikka_gpu::{Compute, ComputeBuffer, Gpu};
use rikka_physics_gpu::{NamedKernel, Pass, Ring, Scan};

fn compute() -> Compute {
    Gpu::builder()
        .app_name("rikka-physics-gpu")
        .compute_support()
        .build()
        .expect("a Vulkan 1.3 device that can compute is required")
        .compute()
        .expect("compute")
}

fn read(compute: &Compute, buffer: &ComputeBuffer, count: usize) -> Vec<u32> {
    let mut batch = compute.batch();
    let download = batch.download_range(buffer, 0, count as u64 * 4);
    batch.submit().expect("the read goes in");
    download
        .wait()
        .expect("the values come back")
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_le_bytes(*word))
        .collect()
}

#[test]
fn scan_computes_exclusive_prefix_sums_across_blocks() {
    let compute = compute();
    let scan = Scan::new(&compute).expect("a scan");
    for count in [1usize, 512, 1024, 1025, 5000] {
        let counts: Vec<u32> = (0..count as u32).map(|at| at * 7 % 13).collect();
        let bytes: Vec<u8> = counts
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        let values = compute.buffer(bytes.len() as u64).expect("values");
        let mut batch = compute.batch();
        batch.write(&values, 0, &bytes);
        scan.run(&mut batch, &values, counts.len() as u32);
        batch.submit().expect("the scan runs");

        let expected: Vec<u32> = counts
            .iter()
            .scan(0u32, |running, value| {
                let start = *running;
                *running += value;
                Some(start)
            })
            .collect();
        assert_eq!(
            read(&compute, &values, counts.len()),
            expected,
            "count={count}"
        );
    }
}

#[test]
fn ring_reuses_only_released_buffers() {
    let compute = compute();
    let mut ring = Ring::new(64);
    let first = ring.acquire(&compute).expect("a buffer");
    let second = ring.acquire(&compute).expect("another buffer");
    assert_ne!(first.address(), second.address());
    let address = first.address();
    drop(first);
    let again = ring.acquire(&compute).expect("a freed buffer");
    assert_eq!(again.address(), address);
}

#[test]
fn pass_refuses_push_constants_its_kernel_does_not_declare() {
    let compute = compute();
    let kernel = NamedKernel::load(&compute, scan_kernels::SCAN_ADD_COMP, "scan_add")
        .expect("the kernel loads");
    let values = compute.buffer(1024).expect("values");
    let sums = compute.buffer(16).expect("sums");
    let mut batch = compute.batch();
    Pass::direct(&kernel, 256, 256)
        .write(&values)
        .read(&sums)
        .run(&mut batch);
    let refused = batch.submit().expect_err("a short push block is refused");
    assert!(refused.to_string().contains("scan_add"), "{refused}");
}
