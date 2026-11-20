use std::arch::x86_64::*;

use super::{blend, from_ps, not, ps, shift_count};
use crate::Simd;
use crate::mask::lane_bits;
use crate::native::ops::{LaneOps, WideLaneOps};
use crate::native::{load_prefix, map_lanes, store_prefix};

macro_rules! vector_mask {
    () => {
        #[inline(always)]
        fn mask_not(mask: __m128i) -> __m128i {
            not(mask)
        }

        #[inline(always)]
        fn mask_all(mask: __m128i) -> bool {
            unsafe { _mm_movemask_epi8(mask) == 0xFFFF }
        }

        #[inline(always)]
        fn select<S: Simd>(mask: __m128i, if_true: __m128i, if_false: __m128i) -> __m128i {
            blend::<S>(mask, if_true, if_false)
        }
    };
}

#[inline(always)]
fn count_bytes<S: Simd>(a: __m128i) -> __m128i {
    unsafe {
        let nibbles = _mm_set1_epi8(0x0F);
        if S::SSE42 {
            let table = _mm_setr_epi8(0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4);
            let low = _mm_and_si128(a, nibbles);
            let high = _mm_and_si128(_mm_srli_epi16::<4>(a), nibbles);
            _mm_add_epi8(_mm_shuffle_epi8(table, low), _mm_shuffle_epi8(table, high))
        } else {
            let pairs = _mm_sub_epi8(
                a,
                _mm_and_si128(_mm_srli_epi16::<1>(a), _mm_set1_epi8(0x55)),
            );
            let quads = _mm_add_epi8(
                _mm_and_si128(pairs, _mm_set1_epi8(0x33)),
                _mm_and_si128(_mm_srli_epi16::<2>(pairs), _mm_set1_epi8(0x33)),
            );
            _mm_and_si128(_mm_add_epi8(quads, _mm_srli_epi16::<4>(quads)), nibbles)
        }
    }
}

#[inline(always)]
fn count_words<S: Simd>(a: __m128i) -> __m128i {
    unsafe {
        let bytes = count_bytes::<S>(a);
        _mm_add_epi16(
            _mm_and_si128(bytes, _mm_set1_epi16(0xFF)),
            _mm_srli_epi16::<8>(bytes),
        )
    }
}

#[inline(always)]
fn byte_shift_left<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
    unsafe {
        if S::AVX512 {
            let low = _mm_set1_epi16(0xFF);
            let even = _mm_and_si128(_mm_sllv_epi16(a, _mm_and_si128(counts, low)), low);
            let odd = _mm_sllv_epi16(_mm_andnot_si128(low, a), _mm_srli_epi16::<8>(counts));
            _mm_or_si128(even, _mm_andnot_si128(low, odd))
        } else {
            map_lanes::<_, u8, 16>(a, counts, |x, n| x.checked_shl(u32::from(n)).unwrap_or(0))
        }
    }
}

#[inline(always)]
fn byte_shift_right<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
    unsafe {
        if S::AVX512 {
            let low = _mm_set1_epi16(0xFF);
            let even = _mm_srlv_epi16(_mm_and_si128(a, low), _mm_and_si128(counts, low));
            let odd = _mm_srlv_epi16(a, _mm_srli_epi16::<8>(counts));
            _mm_or_si128(even, _mm_andnot_si128(low, odd))
        } else {
            map_lanes::<_, u8, 16>(a, counts, |x, n| x.checked_shr(u32::from(n)).unwrap_or(0))
        }
    }
}

#[inline(always)]
pub(super) fn byte_shift_right_arithmetic<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
    unsafe {
        if S::AVX512 {
            let low = _mm_set1_epi16(0xFF);
            let even = _mm_srli_epi16::<8>(_mm_srav_epi16(
                _mm_slli_epi16::<8>(a),
                _mm_and_si128(counts, low),
            ));
            let odd = _mm_srav_epi16(a, _mm_srli_epi16::<8>(counts));
            _mm_or_si128(even, _mm_andnot_si128(low, odd))
        } else {
            map_lanes::<_, i8, 16>(a, counts, |x, n| x >> (n as u8).min(7))
        }
    }
}

impl LaneOps<__m128i> for u8 {
    vector_mask!();

    #[inline(always)]
    fn bitmask(mask: __m128i) -> u64 {
        unsafe { u64::from(_mm_movemask_epi8(mask) as u32) }
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> __m128i {
        unsafe {
            let spread = 0x0101_0101_0101_0101u64;
            let low = (bits & 0xFF).wrapping_mul(spread);
            let high = (bits >> 8 & 0xFF).wrapping_mul(spread);
            let lanes = _mm_set_epi64x(high as i64, low as i64);
            let select = _mm_set1_epi64x(0x8040_2010_0804_0201u64 as i64);
            _mm_cmpeq_epi8(_mm_and_si128(lanes, select), select)
        }
    }

    #[inline(always)]
    fn splat(value: u8) -> __m128i {
        unsafe { _mm_set1_epi8(value as i8) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_cmpeq_epi8(a, b) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_add_epi8(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_sub_epi8(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            let even = _mm_mullo_epi16(a, b);
            let odd = _mm_mullo_epi16(_mm_srli_epi16::<8>(a), _mm_srli_epi16::<8>(b));
            _mm_or_si128(
                _mm_and_si128(even, _mm_set1_epi16(0xFF)),
                _mm_slli_epi16::<8>(odd),
            )
        }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe {
            let byte_mask = 0xFFu32.checked_shl(shift).unwrap_or(0) as u8;
            _mm_and_si128(
                _mm_sll_epi16(a, shift_count(shift)),
                _mm_set1_epi8(byte_mask as i8),
            )
        }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe {
            let byte_mask = 0xFFu32.checked_shr(shift).unwrap_or(0) as u8;
            _mm_and_si128(
                _mm_srl_epi16(a, shift_count(shift)),
                _mm_set1_epi8(byte_mask as i8),
            )
        }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        byte_shift_left::<S>(a, counts)
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        byte_shift_right::<S>(a, counts)
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m128i, b: __m128i) -> (__m128i, __m128i) {
        unsafe { (_mm_unpacklo_epi8(a, b), _mm_unpackhi_epi8(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m128i, b: __m128i) -> (__m128i, __m128i) {
        unsafe {
            let low = _mm_set1_epi16(0xFF);
            (
                _mm_packus_epi16(_mm_and_si128(a, low), _mm_and_si128(b, low)),
                _mm_packus_epi16(_mm_srli_epi16::<8>(a), _mm_srli_epi16::<8>(b)),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                let order = _mm_setr_epi8(15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0);
                _mm_shuffle_epi8(a, order)
            } else {
                let words =
                    _mm_shufflehi_epi16::<0x1B>(_mm_shufflelo_epi16::<0x1B>(_mm_shuffle_epi32::<
                        0x4E,
                    >(
                        a
                    )));
                _mm_or_si128(_mm_slli_epi16::<8>(words), _mm_srli_epi16::<8>(words))
            }
        }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u8, len: usize) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_maskz_loadu_epi8(lane_bits(len) as u16, ptr.cast())
            } else {
                load_prefix::<_, u8, 16>(ptr, len)
            }
        }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m128i, ptr: *mut u8, len: usize) {
        unsafe {
            if S::AVX512 {
                _mm_mask_storeu_epi8(ptr.cast(), lane_bits(len) as u16, value)
            } else {
                store_prefix::<_, u8, 16>(value, ptr, len)
            }
        }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m128i) -> u8 {
        unsafe {
            let sums = _mm_sad_epu8(a, _mm_setzero_si128());
            _mm_cvtsi128_si32(_mm_add_epi64(sums, _mm_unpackhi_epi64(sums, sums))) as u8
        }
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            let a = _mm_add_epi8(a, _mm_slli_si128::<1>(a));
            let a = _mm_add_epi8(a, _mm_slli_si128::<2>(a));
            let a = _mm_add_epi8(a, _mm_slli_si128::<4>(a));
            _mm_add_epi8(a, _mm_slli_si128::<8>(a))
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m128i) -> __m128i {
        count_bytes::<S>(a)
    }
}

impl LaneOps<__m128i> for u16 {
    vector_mask!();

    #[inline(always)]
    fn bitmask(mask: __m128i) -> u64 {
        unsafe { u64::from(_mm_movemask_epi8(_mm_packs_epi16(mask, _mm_setzero_si128())) as u32) }
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> __m128i {
        unsafe {
            let select = _mm_setr_epi16(1, 2, 4, 8, 16, 32, 64, 128);
            _mm_cmpeq_epi16(_mm_and_si128(_mm_set1_epi16(bits as i16), select), select)
        }
    }

    #[inline(always)]
    fn splat(value: u16) -> __m128i {
        unsafe { _mm_set1_epi16(value as i16) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_cmpeq_epi16(a, b) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_add_epi16(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_sub_epi16(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_mullo_epi16(a, b) }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe { _mm_sll_epi16(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe { _mm_srl_epi16(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_sllv_epi16(a, counts)
            } else {
                map_lanes::<_, u16, 8>(a, counts, |x, n| x.checked_shl(u32::from(n)).unwrap_or(0))
            }
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_srlv_epi16(a, counts)
            } else {
                map_lanes::<_, u16, 8>(a, counts, |x, n| x.checked_shr(u32::from(n)).unwrap_or(0))
            }
        }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m128i, b: __m128i) -> (__m128i, __m128i) {
        unsafe { (_mm_unpacklo_epi16(a, b), _mm_unpackhi_epi16(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m128i, b: __m128i) -> (__m128i, __m128i) {
        unsafe {
            let even = |x: __m128i| _mm_srai_epi32::<16>(_mm_slli_epi32::<16>(x));
            (
                _mm_packs_epi32(even(a), even(b)),
                _mm_packs_epi32(_mm_srai_epi32::<16>(a), _mm_srai_epi32::<16>(b)),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            _mm_shuffle_epi32::<0x4E>(_mm_shufflehi_epi16::<0x1B>(_mm_shufflelo_epi16::<0x1B>(a)))
        }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u16, len: usize) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_maskz_loadu_epi16(lane_bits(len) as u8, ptr.cast())
            } else {
                load_prefix::<_, u16, 8>(ptr, len)
            }
        }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m128i, ptr: *mut u16, len: usize) {
        unsafe {
            if S::AVX512 {
                _mm_mask_storeu_epi16(ptr.cast(), lane_bits(len) as u8, value)
            } else {
                store_prefix::<_, u16, 8>(value, ptr, len)
            }
        }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m128i) -> u16 {
        unsafe {
            let a = _mm_add_epi16(a, _mm_srli_si128::<8>(a));
            let a = _mm_add_epi16(a, _mm_srli_si128::<4>(a));
            let a = _mm_add_epi16(a, _mm_srli_si128::<2>(a));
            _mm_cvtsi128_si32(a) as u16
        }
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            let a = _mm_add_epi16(a, _mm_slli_si128::<2>(a));
            let a = _mm_add_epi16(a, _mm_slli_si128::<4>(a));
            _mm_add_epi16(a, _mm_slli_si128::<8>(a))
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m128i) -> __m128i {
        count_words::<S>(a)
    }
}

impl LaneOps<__m128i> for u32 {
    vector_mask!();

    #[inline(always)]
    fn bitmask(mask: __m128i) -> u64 {
        unsafe { u64::from(_mm_movemask_ps(ps(mask)) as u32) }
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> __m128i {
        unsafe {
            let select = _mm_setr_epi32(1, 2, 4, 8);
            _mm_cmpeq_epi32(_mm_and_si128(_mm_set1_epi32(bits as i32), select), select)
        }
    }

    #[inline(always)]
    fn splat(value: u32) -> __m128i {
        unsafe { _mm_set1_epi32(value as i32) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_cmpeq_epi32(a, b) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_add_epi32(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_sub_epi32(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_mullo_epi32(a, b)
            } else {
                let even = _mm_mul_epu32(a, b);
                let odd = _mm_mul_epu32(_mm_srli_epi64::<32>(a), _mm_srli_epi64::<32>(b));
                _mm_unpacklo_epi32(
                    _mm_shuffle_epi32::<0x08>(even),
                    _mm_shuffle_epi32::<0x08>(odd),
                )
            }
        }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe { _mm_sll_epi32(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe { _mm_srl_epi32(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                _mm_sllv_epi32(a, counts)
            } else {
                map_lanes::<_, u32, 4>(a, counts, |x, n| x.checked_shl(n).unwrap_or(0))
            }
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                _mm_srlv_epi32(a, counts)
            } else {
                map_lanes::<_, u32, 4>(a, counts, |x, n| x.checked_shr(n).unwrap_or(0))
            }
        }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m128i, b: __m128i) -> (__m128i, __m128i) {
        unsafe { (_mm_unpacklo_epi32(a, b), _mm_unpackhi_epi32(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m128i, b: __m128i) -> (__m128i, __m128i) {
        unsafe {
            (
                from_ps(_mm_shuffle_ps::<0x88>(ps(a), ps(b))),
                from_ps(_mm_shuffle_ps::<0xDD>(ps(a), ps(b))),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_shuffle_epi32::<0x1B>(a) }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u32, len: usize) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_maskz_loadu_epi32(lane_bits(len) as u8, ptr.cast())
            } else if S::AVX2 {
                _mm_maskload_epi32(
                    ptr.cast(),
                    <Self as LaneOps<__m128i>>::from_bitmask(lane_bits(len)),
                )
            } else {
                load_prefix::<_, u32, 4>(ptr, len)
            }
        }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m128i, ptr: *mut u32, len: usize) {
        unsafe {
            if S::AVX512 {
                _mm_mask_storeu_epi32(ptr.cast(), lane_bits(len) as u8, value)
            } else if S::AVX2 {
                _mm_maskstore_epi32(
                    ptr.cast(),
                    <Self as LaneOps<__m128i>>::from_bitmask(lane_bits(len)),
                    value,
                )
            } else {
                store_prefix::<_, u32, 4>(value, ptr, len)
            }
        }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m128i) -> u32 {
        unsafe {
            let a = _mm_add_epi32(a, _mm_shuffle_epi32::<0x4E>(a));
            _mm_cvtsi128_si32(_mm_add_epi32(a, _mm_shuffle_epi32::<0xB1>(a))) as u32
        }
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            let a = _mm_add_epi32(a, _mm_slli_si128::<4>(a));
            _mm_add_epi32(a, _mm_slli_si128::<8>(a))
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            let words = count_words::<S>(a);
            _mm_add_epi32(
                _mm_and_si128(words, _mm_set1_epi32(0xFFFF)),
                _mm_srli_epi32::<16>(words),
            )
        }
    }
}

impl LaneOps<__m128i> for u64 {
    vector_mask!();

    #[inline(always)]
    fn bitmask(mask: __m128i) -> u64 {
        unsafe { u64::from(_mm_movemask_pd(_mm_castsi128_pd(mask)) as u32) }
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> __m128i {
        unsafe {
            let select = _mm_setr_epi32(1, 1, 2, 2);
            _mm_cmpeq_epi32(_mm_and_si128(_mm_set1_epi32(bits as i32), select), select)
        }
    }

    #[inline(always)]
    fn splat(value: u64) -> __m128i {
        unsafe { _mm_set1_epi64x(value as i64) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_cmpeq_epi64(a, b)
            } else {
                let halves = _mm_cmpeq_epi32(a, b);
                _mm_and_si128(halves, _mm_shuffle_epi32::<0xB1>(halves))
            }
        }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_add_epi64(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_sub_epi64(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_mullo_epi64(a, b)
            } else {
                let low = _mm_mul_epu32(a, b);
                let cross = _mm_add_epi64(
                    _mm_mul_epu32(_mm_srli_epi64::<32>(a), b),
                    _mm_mul_epu32(a, _mm_srli_epi64::<32>(b)),
                );
                _mm_add_epi64(low, _mm_slli_epi64::<32>(cross))
            }
        }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe { _mm_sll_epi64(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe { _mm_srl_epi64(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                _mm_sllv_epi64(a, counts)
            } else {
                map_lanes::<_, u64, 2>(a, counts, |x, n| {
                    u32::try_from(n)
                        .ok()
                        .and_then(|n| x.checked_shl(n))
                        .unwrap_or(0)
                })
            }
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                _mm_srlv_epi64(a, counts)
            } else {
                map_lanes::<_, u64, 2>(a, counts, |x, n| {
                    u32::try_from(n)
                        .ok()
                        .and_then(|n| x.checked_shr(n))
                        .unwrap_or(0)
                })
            }
        }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m128i, b: __m128i) -> (__m128i, __m128i) {
        unsafe { (_mm_unpacklo_epi64(a, b), _mm_unpackhi_epi64(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m128i, b: __m128i) -> (__m128i, __m128i) {
        unsafe { (_mm_unpacklo_epi64(a, b), _mm_unpackhi_epi64(a, b)) }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_shuffle_epi32::<0x4E>(a) }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u64, len: usize) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_maskz_loadu_epi64(lane_bits(len) as u8, ptr.cast())
            } else if S::AVX2 {
                _mm_maskload_epi64(
                    ptr.cast(),
                    <Self as LaneOps<__m128i>>::from_bitmask(lane_bits(len)),
                )
            } else {
                load_prefix::<_, u64, 2>(ptr, len)
            }
        }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m128i, ptr: *mut u64, len: usize) {
        unsafe {
            if S::AVX512 {
                _mm_mask_storeu_epi64(ptr.cast(), lane_bits(len) as u8, value)
            } else if S::AVX2 {
                _mm_maskstore_epi64(
                    ptr.cast(),
                    <Self as LaneOps<__m128i>>::from_bitmask(lane_bits(len)),
                    value,
                )
            } else {
                store_prefix::<_, u64, 2>(value, ptr, len)
            }
        }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m128i) -> u64 {
        unsafe { _mm_cvtsi128_si64(_mm_add_epi64(a, _mm_shuffle_epi32::<0x4E>(a))) as u64 }
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_add_epi64(a, _mm_slli_si128::<8>(a)) }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_sad_epu8(count_bytes::<S>(a), _mm_setzero_si128()) }
    }
}

const fn compress_table<const ENTRIES: usize>(lane_bytes: usize) -> [[u8; 16]; ENTRIES] {
    let lanes = 16 / lane_bytes;
    let mut table = [[0x80u8; 16]; ENTRIES];
    let mut mask = 0;
    while mask < ENTRIES {
        let mut filled = 0;
        let mut lane = 0;
        while lane < lanes {
            if mask >> lane & 1 == 1 {
                let mut byte = 0;
                while byte < lane_bytes {
                    table[mask][filled * lane_bytes + byte] = (lane * lane_bytes + byte) as u8;
                    byte += 1;
                }
                filled += 1;
            }
            lane += 1;
        }
        mask += 1;
    }
    table
}

static COMPRESS_32: [[u8; 16]; 16] = compress_table::<16>(4);
static COMPRESS_64: [[u8; 16]; 4] = compress_table::<4>(8);

#[inline(always)]
fn compress_bytes(a: __m128i, order: &[u8; 16]) -> __m128i {
    unsafe { _mm_shuffle_epi8(a, _mm_loadu_si128(order.as_ptr().cast())) }
}

impl WideLaneOps<__m128i> for u32 {
    #[inline(always)]
    unsafe fn gather<S: Simd>(base: *const u32, indices: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                _mm_i32gather_epi32::<4>(base.cast(), indices)
            } else {
                let indices: [u32; 4] = crate::native::to_lanes(indices);
                crate::native::from_lanes(indices.map(|index| *base.add(index as usize)))
            }
        }
    }

    #[inline(always)]
    fn compress<S: Simd>(a: __m128i, mask: __m128i) -> __m128i {
        unsafe {
            let bits = <Self as LaneOps<__m128i>>::bitmask(mask);
            if S::AVX512 {
                _mm_maskz_compress_epi32(bits as u8, a)
            } else if S::SSE42 {
                compress_bytes(a, &COMPRESS_32[bits as usize])
            } else {
                let lanes: [u32; 4] = crate::native::to_lanes(a);
                let mut packed = [0u32; 4];
                let mut filled = 0;
                for (lane, value) in lanes.into_iter().enumerate() {
                    if bits >> lane & 1 == 1 {
                        packed[filled] = value;
                        filled += 1;
                    }
                }
                crate::native::from_lanes(packed)
            }
        }
    }
}

impl WideLaneOps<__m128i> for u64 {
    #[inline(always)]
    unsafe fn gather<S: Simd>(base: *const u64, indices: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                _mm_i64gather_epi64::<8>(base.cast(), indices)
            } else {
                let indices: [u64; 2] = crate::native::to_lanes(indices);
                crate::native::from_lanes(indices.map(|index| *base.add(index as usize)))
            }
        }
    }

    #[inline(always)]
    fn compress<S: Simd>(a: __m128i, mask: __m128i) -> __m128i {
        unsafe {
            let bits = <Self as LaneOps<__m128i>>::bitmask(mask);
            if S::AVX512 {
                _mm_maskz_compress_epi64(bits as u8, a)
            } else if S::SSE42 {
                compress_bytes(a, &COMPRESS_64[bits as usize])
            } else {
                match bits {
                    0 => _mm_setzero_si128(),
                    1 => _mm_move_epi64(a),
                    2 => _mm_unpackhi_epi64(a, _mm_setzero_si128()),
                    _ => a,
                }
            }
        }
    }
}
