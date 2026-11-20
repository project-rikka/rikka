mod float;
mod int;
mod lanes;

use std::arch::x86_64::*;

use super::from_lanes;
use super::ops::Register;

impl Register for __m512i {
    type Mask = u64;
    const BYTES: usize = 64;

    #[inline(always)]
    fn zero() -> Self {
        unsafe { _mm512_setzero_si512() }
    }

    #[inline(always)]
    fn ones() -> Self {
        unsafe { _mm512_set1_epi32(-1) }
    }

    #[inline(always)]
    unsafe fn load(ptr: *const u8) -> Self {
        unsafe { _mm512_loadu_si512(ptr.cast()) }
    }

    #[inline(always)]
    unsafe fn store(self, ptr: *mut u8) {
        unsafe { _mm512_storeu_si512(ptr.cast(), self) }
    }

    #[inline(always)]
    fn and(self, other: Self) -> Self {
        unsafe { _mm512_and_si512(self, other) }
    }

    #[inline(always)]
    fn or(self, other: Self) -> Self {
        unsafe { _mm512_or_si512(self, other) }
    }

    #[inline(always)]
    fn xor(self, other: Self) -> Self {
        unsafe { _mm512_xor_si512(self, other) }
    }

    #[inline(always)]
    fn mask_and(a: u64, b: u64) -> u64 {
        a & b
    }

    #[inline(always)]
    fn mask_or(a: u64, b: u64) -> u64 {
        a | b
    }

    #[inline(always)]
    fn mask_xor(a: u64, b: u64) -> u64 {
        a ^ b
    }

    #[inline(always)]
    fn mask_any(mask: u64) -> bool {
        mask != 0
    }
}

pub(super) use super::shift_count;

#[inline(always)]
pub(super) fn ps(raw: __m512i) -> __m512 {
    unsafe { _mm512_castsi512_ps(raw) }
}

#[inline(always)]
pub(super) fn pd(raw: __m512i) -> __m512d {
    unsafe { _mm512_castsi512_pd(raw) }
}

#[inline(always)]
pub(super) fn from_ps(raw: __m512) -> __m512i {
    unsafe { _mm512_castps_si512(raw) }
}

#[inline(always)]
pub(super) fn from_pd(raw: __m512d) -> __m512i {
    unsafe { _mm512_castpd_si512(raw) }
}

#[inline(always)]
pub(super) fn low(raw: __m512i) -> __m256i {
    unsafe { _mm512_castsi512_si256(raw) }
}

#[inline(always)]
pub(super) fn high(raw: __m512i) -> __m256i {
    unsafe { _mm512_extracti64x4_epi64::<1>(raw) }
}

#[inline(always)]
pub(super) fn join(low: __m256i, high: __m256i) -> __m512i {
    unsafe { _mm512_inserti64x4::<1>(_mm512_castsi256_si512(low), high) }
}

#[inline(always)]
pub(super) fn qwords(order: [i64; 8]) -> __m512i {
    from_lanes(order)
}

#[inline(always)]
pub(super) fn carry_blocks(
    sums: __m512i,
    last_of_block: impl Fn(__m512i) -> __m512i,
    add: impl Fn(__m512i, __m512i) -> __m512i,
) -> __m512i {
    unsafe {
        let last = last_of_block(sums);
        let sums = add(sums, _mm512_maskz_shuffle_i32x4::<0xA0>(0xF0F0, last, last));
        let last = last_of_block(sums);
        add(sums, _mm512_maskz_shuffle_i32x4::<0x50>(0xFF00, last, last))
    }
}
