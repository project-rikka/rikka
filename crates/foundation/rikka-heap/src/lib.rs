pub use mimalloc::MiMalloc as Mimalloc;

#[cfg(target_os = "linux")]
const LARGEST_POOLED_BLOCK: libc::c_int = 128 * 1024;

#[macro_export]
macro_rules! mimalloc {
    () => {
        #[global_allocator]
        static RUST_HEAP: $crate::Mimalloc = $crate::Mimalloc;
    };
}

#[cfg(target_os = "linux")]
pub fn return_large_glibc_blocks() {
    unsafe { libc::mallopt(libc::M_MMAP_THRESHOLD, LARGEST_POOLED_BLOCK) };
}

#[cfg(target_os = "linux")]
pub fn return_free_glibc_memory() {
    unsafe { libc::malloc_trim(0) };
}
