mod float;
mod int;
mod lanes;

use std::arch::x86_64::*;

use super::ops::Register;

impl Register for __m256i {
    type Mask = __m256i;
    const BYTES: usize = 32;

    #[inline(always)]
    fn zero() -> Self {
        unsafe { _mm256_setzero_si256() }
    }

    #[inline(always)]
    fn ones() -> Self {
        unsafe { _mm256_set1_epi32(-1) }
    }

    #[inline(always)]
    unsafe fn load(ptr: *const u8) -> Self {
        unsafe { _mm256_loadu_si256(ptr.cast()) }
    }

    #[inline(always)]
    unsafe fn store(self, ptr: *mut u8) {
        unsafe { _mm256_storeu_si256(ptr.cast(), self) }
    }

    #[inline(always)]
    fn and(self, other: Self) -> Self {
        unsafe { _mm256_and_si256(self, other) }
    }

    #[inline(always)]
    fn or(self, other: Self) -> Self {
        unsafe { _mm256_or_si256(self, other) }
    }

    #[inline(always)]
    fn xor(self, other: Self) -> Self {
        unsafe { _mm256_xor_si256(self, other) }
    }

    #[inline(always)]
    fn mask_and(a: Self, b: Self) -> Self {
        unsafe { _mm256_and_si256(a, b) }
    }

    #[inline(always)]
    fn mask_or(a: Self, b: Self) -> Self {
        unsafe { _mm256_or_si256(a, b) }
    }

    #[inline(always)]
    fn mask_xor(a: Self, b: Self) -> Self {
        unsafe { _mm256_xor_si256(a, b) }
    }

    #[inline(always)]
    fn mask_any(mask: Self) -> bool {
        unsafe { _mm256_testz_si256(mask, mask) == 0 }
    }
}

#[inline(always)]
pub(super) fn blend(mask: __m256i, if_true: __m256i, if_false: __m256i) -> __m256i {
    unsafe { _mm256_blendv_epi8(if_false, if_true, mask) }
}

pub(super) use super::shift_count;

#[inline(always)]
pub(super) fn ps(raw: __m256i) -> __m256 {
    unsafe { _mm256_castsi256_ps(raw) }
}

#[inline(always)]
pub(super) fn pd(raw: __m256i) -> __m256d {
    unsafe { _mm256_castsi256_pd(raw) }
}

#[inline(always)]
pub(super) fn from_ps(raw: __m256) -> __m256i {
    unsafe { _mm256_castps_si256(raw) }
}

#[inline(always)]
pub(super) fn from_pd(raw: __m256d) -> __m256i {
    unsafe { _mm256_castpd_si256(raw) }
}

#[inline(always)]
pub(super) fn not(raw: __m256i) -> __m256i {
    unsafe { _mm256_xor_si256(raw, _mm256_set1_epi32(-1)) }
}

#[inline(always)]
pub(super) fn low(raw: __m256i) -> __m128i {
    unsafe { _mm256_castsi256_si128(raw) }
}

#[inline(always)]
pub(super) fn high(raw: __m256i) -> __m128i {
    unsafe { _mm256_extracti128_si256::<1>(raw) }
}

#[inline(always)]
pub(super) fn join(low: __m128i, high: __m128i) -> __m256i {
    unsafe { _mm256_set_m128i(high, low) }
}

#[inline(always)]
pub(super) fn into_high_block(last: __m256i) -> __m256i {
    unsafe { _mm256_permute2x128_si256::<0x08>(last, last) }
}

pub(super) const LANE_ORDER: i32 = 0xD8;
