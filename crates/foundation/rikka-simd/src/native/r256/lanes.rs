use std::arch::x86_64::*;

use super::{LANE_ORDER, blend, from_ps, high, into_high_block, low, not, ps, shift_count};
use crate::Simd;
use crate::mask::lane_bits;
use crate::native::ops::{LaneOps, WideLaneOps};
use crate::native::{load_prefix, map_lanes, store_prefix};

macro_rules! vector_mask {
    () => {
        #[inline(always)]
        fn mask_not(mask: __m256i) -> __m256i {
            not(mask)
        }

        #[inline(always)]
        fn mask_all(mask: __m256i) -> bool {
            unsafe { _mm256_movemask_epi8(mask) == -1 }
        }

        #[inline(always)]
        fn select<S: Simd>(mask: __m256i, if_true: __m256i, if_false: __m256i) -> __m256i {
            blend(mask, if_true, if_false)
        }
    };
}

#[inline(always)]
fn count_bytes(a: __m256i) -> __m256i {
    unsafe {
        let nibbles = _mm256_set1_epi8(0x0F);
        let table = _mm256_setr_epi8(
            0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2,
            3, 3, 4,
        );
        let low_nibbles = _mm256_and_si256(a, nibbles);
        let high_nibbles = _mm256_and_si256(_mm256_srli_epi16::<4>(a), nibbles);
        _mm256_add_epi8(
            _mm256_shuffle_epi8(table, low_nibbles),
            _mm256_shuffle_epi8(table, high_nibbles),
        )
    }
}

#[inline(always)]
fn count_words(a: __m256i) -> __m256i {
    unsafe {
        let bytes = count_bytes(a);
        _mm256_add_epi16(
            _mm256_and_si256(bytes, _mm256_set1_epi16(0xFF)),
            _mm256_srli_epi16::<8>(bytes),
        )
    }
}

#[inline(always)]
fn zip(low_lanes: __m256i, high_lanes: __m256i) -> (__m256i, __m256i) {
    unsafe {
        (
            _mm256_permute2x128_si256::<0x20>(low_lanes, high_lanes),
            _mm256_permute2x128_si256::<0x31>(low_lanes, high_lanes),
        )
    }
}

#[inline(always)]
pub(super) fn byte_shift_right_arithmetic<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
    if S::AVX512 {
        unsafe {
            let low_bytes = _mm256_set1_epi16(0xFF);
            let even = _mm256_srli_epi16::<8>(_mm256_srav_epi16(
                _mm256_slli_epi16::<8>(a),
                _mm256_and_si256(counts, low_bytes),
            ));
            let odd = _mm256_srav_epi16(a, _mm256_srli_epi16::<8>(counts));
            _mm256_or_si256(even, _mm256_andnot_si256(low_bytes, odd))
        }
    } else {
        map_lanes::<_, i8, 32>(a, counts, |x, n| x >> (n as u8).min(7))
    }
}

impl LaneOps<__m256i> for u8 {
    vector_mask!();

    #[inline(always)]
    fn bitmask(mask: __m256i) -> u64 {
        unsafe { u64::from(_mm256_movemask_epi8(mask) as u32) }
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> __m256i {
        unsafe {
            let spread = _mm256_shuffle_epi8(
                _mm256_set1_epi32(bits as i32),
                _mm256_setr_epi8(
                    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3,
                    3, 3, 3, 3, 3, 3,
                ),
            );
            let select = _mm256_set1_epi64x(0x8040_2010_0804_0201u64 as i64);
            _mm256_cmpeq_epi8(_mm256_and_si256(spread, select), select)
        }
    }

    #[inline(always)]
    fn splat(value: u8) -> __m256i {
        unsafe { _mm256_set1_epi8(value as i8) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_cmpeq_epi8(a, b) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_add_epi8(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_sub_epi8(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            let even = _mm256_mullo_epi16(a, b);
            let odd = _mm256_mullo_epi16(_mm256_srli_epi16::<8>(a), _mm256_srli_epi16::<8>(b));
            _mm256_or_si256(
                _mm256_and_si256(even, _mm256_set1_epi16(0xFF)),
                _mm256_slli_epi16::<8>(odd),
            )
        }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        let byte_mask = 0xFFu32.checked_shl(shift).unwrap_or(0) as u8;
        unsafe {
            _mm256_and_si256(
                _mm256_sll_epi16(a, shift_count(shift)),
                _mm256_set1_epi8(byte_mask as i8),
            )
        }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        let byte_mask = 0xFFu32.checked_shr(shift).unwrap_or(0) as u8;
        unsafe {
            _mm256_and_si256(
                _mm256_srl_epi16(a, shift_count(shift)),
                _mm256_set1_epi8(byte_mask as i8),
            )
        }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe {
                let low_bytes = _mm256_set1_epi16(0xFF);
                let even = _mm256_and_si256(
                    _mm256_sllv_epi16(a, _mm256_and_si256(counts, low_bytes)),
                    low_bytes,
                );
                let odd = _mm256_sllv_epi16(
                    _mm256_andnot_si256(low_bytes, a),
                    _mm256_srli_epi16::<8>(counts),
                );
                _mm256_or_si256(even, _mm256_andnot_si256(low_bytes, odd))
            }
        } else {
            map_lanes::<_, u8, 32>(a, counts, |x, n| x.checked_shl(u32::from(n)).unwrap_or(0))
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe {
                let low_bytes = _mm256_set1_epi16(0xFF);
                let even = _mm256_srlv_epi16(
                    _mm256_and_si256(a, low_bytes),
                    _mm256_and_si256(counts, low_bytes),
                );
                let odd = _mm256_srlv_epi16(a, _mm256_srli_epi16::<8>(counts));
                _mm256_or_si256(even, _mm256_andnot_si256(low_bytes, odd))
            }
        } else {
            map_lanes::<_, u8, 32>(a, counts, |x, n| x.checked_shr(u32::from(n)).unwrap_or(0))
        }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m256i, b: __m256i) -> (__m256i, __m256i) {
        unsafe { zip(_mm256_unpacklo_epi8(a, b), _mm256_unpackhi_epi8(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m256i, b: __m256i) -> (__m256i, __m256i) {
        unsafe {
            let low_bytes = _mm256_set1_epi16(0xFF);
            let even = _mm256_packus_epi16(
                _mm256_and_si256(a, low_bytes),
                _mm256_and_si256(b, low_bytes),
            );
            let odd = _mm256_packus_epi16(_mm256_srli_epi16::<8>(a), _mm256_srli_epi16::<8>(b));
            (
                _mm256_permute4x64_epi64::<LANE_ORDER>(even),
                _mm256_permute4x64_epi64::<LANE_ORDER>(odd),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            let order = _mm256_setr_epi8(
                15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 15, 14, 13, 12, 11, 10, 9, 8,
                7, 6, 5, 4, 3, 2, 1, 0,
            );
            _mm256_permute4x64_epi64::<0x4E>(_mm256_shuffle_epi8(a, order))
        }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u8, len: usize) -> __m256i {
        if S::AVX512 {
            unsafe { _mm256_maskz_loadu_epi8(lane_bits(len) as u32, ptr.cast()) }
        } else {
            unsafe { load_prefix::<_, u8, 32>(ptr, len) }
        }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m256i, ptr: *mut u8, len: usize) {
        if S::AVX512 {
            unsafe { _mm256_mask_storeu_epi8(ptr.cast(), lane_bits(len) as u32, value) }
        } else {
            unsafe { store_prefix::<_, u8, 32>(value, ptr, len) }
        }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m256i) -> u8 {
        unsafe { <u8 as LaneOps<__m128i>>::reduce_add::<S>(_mm_add_epi8(low(a), high(a))) }
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            let a = _mm256_add_epi8(a, _mm256_slli_si256::<1>(a));
            let a = _mm256_add_epi8(a, _mm256_slli_si256::<2>(a));
            let a = _mm256_add_epi8(a, _mm256_slli_si256::<4>(a));
            let a = _mm256_add_epi8(a, _mm256_slli_si256::<8>(a));
            let last = _mm256_shuffle_epi8(a, _mm256_set1_epi8(15));
            _mm256_add_epi8(a, into_high_block(last))
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m256i) -> __m256i {
        count_bytes(a)
    }
}

impl LaneOps<__m256i> for u16 {
    vector_mask!();

    #[inline(always)]
    fn bitmask(mask: __m256i) -> u64 {
        unsafe { u64::from(_mm_movemask_epi8(_mm_packs_epi16(low(mask), high(mask))) as u16) }
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> __m256i {
        unsafe {
            let select = _mm256_setr_epi16(
                1,
                2,
                4,
                8,
                16,
                32,
                64,
                128,
                256,
                512,
                1024,
                2048,
                4096,
                8192,
                16384,
                i16::MIN,
            );
            _mm256_cmpeq_epi16(
                _mm256_and_si256(_mm256_set1_epi16(bits as i16), select),
                select,
            )
        }
    }

    #[inline(always)]
    fn splat(value: u16) -> __m256i {
        unsafe { _mm256_set1_epi16(value as i16) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_cmpeq_epi16(a, b) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_add_epi16(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_sub_epi16(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_mullo_epi16(a, b) }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        unsafe { _mm256_sll_epi16(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        unsafe { _mm256_srl_epi16(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe { _mm256_sllv_epi16(a, counts) }
        } else {
            map_lanes::<_, u16, 16>(a, counts, |x, n| x.checked_shl(u32::from(n)).unwrap_or(0))
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe { _mm256_srlv_epi16(a, counts) }
        } else {
            map_lanes::<_, u16, 16>(a, counts, |x, n| x.checked_shr(u32::from(n)).unwrap_or(0))
        }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m256i, b: __m256i) -> (__m256i, __m256i) {
        unsafe { zip(_mm256_unpacklo_epi16(a, b), _mm256_unpackhi_epi16(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m256i, b: __m256i) -> (__m256i, __m256i) {
        unsafe {
            let low_words = _mm256_set1_epi32(0xFFFF);
            let even = _mm256_packus_epi32(
                _mm256_and_si256(a, low_words),
                _mm256_and_si256(b, low_words),
            );
            let odd = _mm256_packus_epi32(_mm256_srli_epi32::<16>(a), _mm256_srli_epi32::<16>(b));
            (
                _mm256_permute4x64_epi64::<LANE_ORDER>(even),
                _mm256_permute4x64_epi64::<LANE_ORDER>(odd),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            let order = _mm256_setr_epi8(
                14, 15, 12, 13, 10, 11, 8, 9, 6, 7, 4, 5, 2, 3, 0, 1, 14, 15, 12, 13, 10, 11, 8, 9,
                6, 7, 4, 5, 2, 3, 0, 1,
            );
            _mm256_permute4x64_epi64::<0x4E>(_mm256_shuffle_epi8(a, order))
        }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u16, len: usize) -> __m256i {
        if S::AVX512 {
            unsafe { _mm256_maskz_loadu_epi16(lane_bits(len) as u16, ptr.cast()) }
        } else {
            unsafe { load_prefix::<_, u16, 16>(ptr, len) }
        }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m256i, ptr: *mut u16, len: usize) {
        if S::AVX512 {
            unsafe { _mm256_mask_storeu_epi16(ptr.cast(), lane_bits(len) as u16, value) }
        } else {
            unsafe { store_prefix::<_, u16, 16>(value, ptr, len) }
        }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m256i) -> u16 {
        unsafe { <u16 as LaneOps<__m128i>>::reduce_add::<S>(_mm_add_epi16(low(a), high(a))) }
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            let a = _mm256_add_epi16(a, _mm256_slli_si256::<2>(a));
            let a = _mm256_add_epi16(a, _mm256_slli_si256::<4>(a));
            let a = _mm256_add_epi16(a, _mm256_slli_si256::<8>(a));
            let last = _mm256_shuffle_epi8(a, _mm256_set1_epi16(0x0F0E));
            _mm256_add_epi16(a, into_high_block(last))
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m256i) -> __m256i {
        count_words(a)
    }
}

impl LaneOps<__m256i> for u32 {
    vector_mask!();

    #[inline(always)]
    fn bitmask(mask: __m256i) -> u64 {
        unsafe { u64::from(_mm256_movemask_ps(ps(mask)) as u32) }
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> __m256i {
        unsafe {
            let select = _mm256_setr_epi32(1, 2, 4, 8, 16, 32, 64, 128);
            _mm256_cmpeq_epi32(
                _mm256_and_si256(_mm256_set1_epi32(bits as i32), select),
                select,
            )
        }
    }

    #[inline(always)]
    fn splat(value: u32) -> __m256i {
        unsafe { _mm256_set1_epi32(value as i32) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_cmpeq_epi32(a, b) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_add_epi32(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_sub_epi32(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_mullo_epi32(a, b) }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        unsafe { _mm256_sll_epi32(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        unsafe { _mm256_srl_epi32(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        unsafe { _mm256_sllv_epi32(a, counts) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        unsafe { _mm256_srlv_epi32(a, counts) }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m256i, b: __m256i) -> (__m256i, __m256i) {
        unsafe { zip(_mm256_unpacklo_epi32(a, b), _mm256_unpackhi_epi32(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m256i, b: __m256i) -> (__m256i, __m256i) {
        unsafe {
            let even = from_ps(_mm256_shuffle_ps::<0x88>(ps(a), ps(b)));
            let odd = from_ps(_mm256_shuffle_ps::<0xDD>(ps(a), ps(b)));
            (
                _mm256_permute4x64_epi64::<LANE_ORDER>(even),
                _mm256_permute4x64_epi64::<LANE_ORDER>(odd),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m256i) -> __m256i {
        unsafe { _mm256_permutevar8x32_epi32(a, _mm256_setr_epi32(7, 6, 5, 4, 3, 2, 1, 0)) }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u32, len: usize) -> __m256i {
        if S::AVX512 {
            unsafe { _mm256_maskz_loadu_epi32(lane_bits(len) as u8, ptr.cast()) }
        } else {
            unsafe {
                _mm256_maskload_epi32(
                    ptr.cast(),
                    <Self as LaneOps<__m256i>>::from_bitmask(lane_bits(len)),
                )
            }
        }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m256i, ptr: *mut u32, len: usize) {
        if S::AVX512 {
            unsafe { _mm256_mask_storeu_epi32(ptr.cast(), lane_bits(len) as u8, value) }
        } else {
            unsafe {
                _mm256_maskstore_epi32(
                    ptr.cast(),
                    <Self as LaneOps<__m256i>>::from_bitmask(lane_bits(len)),
                    value,
                )
            }
        }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m256i) -> u32 {
        unsafe { <u32 as LaneOps<__m128i>>::reduce_add::<S>(_mm_add_epi32(low(a), high(a))) }
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            let a = _mm256_add_epi32(a, _mm256_slli_si256::<4>(a));
            let a = _mm256_add_epi32(a, _mm256_slli_si256::<8>(a));
            let last = _mm256_shuffle_epi32::<0xFF>(a);
            _mm256_add_epi32(a, into_high_block(last))
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            let words = count_words(a);
            _mm256_add_epi32(
                _mm256_and_si256(words, _mm256_set1_epi32(0xFFFF)),
                _mm256_srli_epi32::<16>(words),
            )
        }
    }
}

impl LaneOps<__m256i> for u64 {
    vector_mask!();

    #[inline(always)]
    fn bitmask(mask: __m256i) -> u64 {
        unsafe { u64::from(_mm256_movemask_pd(_mm256_castsi256_pd(mask)) as u32) }
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> __m256i {
        unsafe {
            let select = _mm256_setr_epi64x(1, 2, 4, 8);
            _mm256_cmpeq_epi64(
                _mm256_and_si256(_mm256_set1_epi64x(bits as i64), select),
                select,
            )
        }
    }

    #[inline(always)]
    fn splat(value: u64) -> __m256i {
        unsafe { _mm256_set1_epi64x(value as i64) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_cmpeq_epi64(a, b) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_add_epi64(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_sub_epi64(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            if S::AVX512 {
                _mm256_mullo_epi64(a, b)
            } else {
                let low_products = _mm256_mul_epu32(a, b);
                let cross = _mm256_add_epi64(
                    _mm256_mul_epu32(_mm256_srli_epi64::<32>(a), b),
                    _mm256_mul_epu32(a, _mm256_srli_epi64::<32>(b)),
                );
                _mm256_add_epi64(low_products, _mm256_slli_epi64::<32>(cross))
            }
        }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        unsafe { _mm256_sll_epi64(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        unsafe { _mm256_srl_epi64(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        unsafe { _mm256_sllv_epi64(a, counts) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        unsafe { _mm256_srlv_epi64(a, counts) }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m256i, b: __m256i) -> (__m256i, __m256i) {
        unsafe { zip(_mm256_unpacklo_epi64(a, b), _mm256_unpackhi_epi64(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m256i, b: __m256i) -> (__m256i, __m256i) {
        unsafe {
            (
                _mm256_permute4x64_epi64::<LANE_ORDER>(_mm256_unpacklo_epi64(a, b)),
                _mm256_permute4x64_epi64::<LANE_ORDER>(_mm256_unpackhi_epi64(a, b)),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m256i) -> __m256i {
        unsafe { _mm256_permute4x64_epi64::<0x1B>(a) }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u64, len: usize) -> __m256i {
        if S::AVX512 {
            unsafe { _mm256_maskz_loadu_epi64(lane_bits(len) as u8, ptr.cast()) }
        } else {
            unsafe {
                _mm256_maskload_epi64(
                    ptr.cast(),
                    <Self as LaneOps<__m256i>>::from_bitmask(lane_bits(len)),
                )
            }
        }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m256i, ptr: *mut u64, len: usize) {
        if S::AVX512 {
            unsafe { _mm256_mask_storeu_epi64(ptr.cast(), lane_bits(len) as u8, value) }
        } else {
            unsafe {
                _mm256_maskstore_epi64(
                    ptr.cast(),
                    <Self as LaneOps<__m256i>>::from_bitmask(lane_bits(len)),
                    value,
                )
            }
        }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m256i) -> u64 {
        unsafe { <u64 as LaneOps<__m128i>>::reduce_add::<S>(_mm_add_epi64(low(a), high(a))) }
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            let a = _mm256_add_epi64(a, _mm256_slli_si256::<8>(a));
            let last = _mm256_shuffle_epi32::<0xEE>(a);
            _mm256_add_epi64(a, into_high_block(last))
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m256i) -> __m256i {
        unsafe { _mm256_sad_epu8(count_bytes(a), _mm256_setzero_si256()) }
    }
}

const fn compress_orders() -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut mask = 0;
    while mask < 256 {
        let mut filled = 0;
        let mut lane = 0;
        while lane < 8 {
            if mask >> lane & 1 == 1 {
                table[mask] |= (lane as u64) << (8 * filled);
                filled += 1;
            }
            lane += 1;
        }
        mask += 1;
    }
    table
}

static COMPRESS_ORDERS: [u64; 256] = compress_orders();

#[inline(always)]
fn compress_by_bits(a: __m256i, bits: u64) -> __m256i {
    unsafe {
        let order = COMPRESS_ORDERS[(bits & 0xFF) as usize];
        let packed =
            _mm256_permutevar8x32_epi32(a, _mm256_cvtepu8_epi32(_mm_cvtsi64_si128(order as i64)));
        let filled = _mm256_set1_epi32(bits.count_ones() as i32);
        let occupied = _mm256_cmpgt_epi32(filled, _mm256_setr_epi32(0, 1, 2, 3, 4, 5, 6, 7));
        _mm256_and_si256(packed, occupied)
    }
}

impl WideLaneOps<__m256i> for u32 {
    #[inline(always)]
    unsafe fn gather<S: Simd>(base: *const u32, indices: __m256i) -> __m256i {
        unsafe { _mm256_i32gather_epi32::<4>(base.cast(), indices) }
    }

    #[inline(always)]
    fn compress<S: Simd>(a: __m256i, mask: __m256i) -> __m256i {
        let bits = <Self as LaneOps<__m256i>>::bitmask(mask);
        if S::AVX512 {
            unsafe { _mm256_maskz_compress_epi32(bits as u8, a) }
        } else {
            compress_by_bits(a, bits)
        }
    }
}

impl WideLaneOps<__m256i> for u64 {
    #[inline(always)]
    unsafe fn gather<S: Simd>(base: *const u64, indices: __m256i) -> __m256i {
        unsafe { _mm256_i64gather_epi64::<8>(base.cast(), indices) }
    }

    #[inline(always)]
    fn compress<S: Simd>(a: __m256i, mask: __m256i) -> __m256i {
        let bits = <Self as LaneOps<__m256i>>::bitmask(mask);
        if S::AVX512 {
            unsafe { _mm256_maskz_compress_epi64(bits as u8, a) }
        } else {
            let pairs = (0..4).fold(0, |pairs, lane| {
                pairs | ((bits >> lane & 1) * 3) << (2 * lane)
            });
            compress_by_bits(a, pairs)
        }
    }
}
