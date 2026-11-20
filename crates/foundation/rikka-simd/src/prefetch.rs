use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};

const CACHE_LINE: usize = 64;

#[inline(always)]
pub fn prefetch<T: ?Sized>(data: *const T, bytes: usize) {
    let base = data.cast::<i8>();
    let mut offset = 0;
    while offset < bytes {
        unsafe { _mm_prefetch::<_MM_HINT_T0>(base.wrapping_add(offset)) };
        offset += CACHE_LINE;
    }
}
