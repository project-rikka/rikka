mod float;
mod int;
mod lanes;

use std::arch::x86_64::*;

use super::ops::{Register, Supports};
use crate::Simd;

unsafe impl<S: Simd> Supports<__m128i> for S {}

impl Register for __m128i {
    type Mask = __m128i;
    const BYTES: usize = 16;

    #[inline(always)]
    fn zero() -> Self {
        unsafe { _mm_setzero_si128() }
    }

    #[inline(always)]
    fn ones() -> Self {
        unsafe { _mm_set1_epi32(-1) }
    }

    #[inline(always)]
    unsafe fn load(ptr: *const u8) -> Self {
        unsafe { _mm_loadu_si128(ptr.cast()) }
    }

    #[inline(always)]
    unsafe fn store(self, ptr: *mut u8) {
        unsafe { _mm_storeu_si128(ptr.cast(), self) }
    }

    #[inline(always)]
    fn and(self, other: Self) -> Self {
        unsafe { _mm_and_si128(self, other) }
    }

    #[inline(always)]
    fn or(self, other: Self) -> Self {
        unsafe { _mm_or_si128(self, other) }
    }

    #[inline(always)]
    fn xor(self, other: Self) -> Self {
        unsafe { _mm_xor_si128(self, other) }
    }

    #[inline(always)]
    fn mask_and(a: Self, b: Self) -> Self {
        unsafe { _mm_and_si128(a, b) }
    }

    #[inline(always)]
    fn mask_or(a: Self, b: Self) -> Self {
        unsafe { _mm_or_si128(a, b) }
    }

    #[inline(always)]
    fn mask_xor(a: Self, b: Self) -> Self {
        unsafe { _mm_xor_si128(a, b) }
    }

    #[inline(always)]
    fn mask_any(mask: Self) -> bool {
        unsafe { _mm_movemask_epi8(mask) != 0 }
    }
}

#[inline(always)]
pub(super) fn blend<S: Simd>(mask: __m128i, if_true: __m128i, if_false: __m128i) -> __m128i {
    unsafe {
        if S::SSE42 {
            _mm_blendv_epi8(if_false, if_true, mask)
        } else {
            _mm_or_si128(
                _mm_and_si128(mask, if_true),
                _mm_andnot_si128(mask, if_false),
            )
        }
    }
}

pub(super) use super::shift_count;

#[inline(always)]
pub(super) fn ps(raw: __m128i) -> __m128 {
    unsafe { _mm_castsi128_ps(raw) }
}

#[inline(always)]
pub(super) fn pd(raw: __m128i) -> __m128d {
    unsafe { _mm_castsi128_pd(raw) }
}

#[inline(always)]
pub(super) fn from_ps(raw: __m128) -> __m128i {
    unsafe { _mm_castps_si128(raw) }
}

#[inline(always)]
pub(super) fn from_pd(raw: __m128d) -> __m128i {
    unsafe { _mm_castpd_si128(raw) }
}

#[inline(always)]
pub(super) fn not(raw: __m128i) -> __m128i {
    unsafe { _mm_xor_si128(raw, _mm_set1_epi32(-1)) }
}
