use std::hint::black_box;

const MIB: usize = 1024 * 1024;

fn mapped_bytes() -> usize {
    unsafe { libc::mallinfo2() }.hblkhd
}

#[test]
fn large_block_returns_to_system() {
    rikka_heap::return_large_glibc_blocks();

    drop(black_box(vec![1u8; 8 * MIB]));
    let before = mapped_bytes();
    let block = black_box(vec![1u8; MIB]);

    assert!(mapped_bytes() >= before + MIB);
    drop(block);
    assert_eq!(mapped_bytes(), before);
}
