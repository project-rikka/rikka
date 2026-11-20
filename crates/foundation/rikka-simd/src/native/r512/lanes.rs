use std::arch::x86_64::*;

use super::{carry_blocks, high, low, qwords, shift_count};
use crate::Simd;
use crate::mask::lane_bits;
use crate::native::from_lanes;
use crate::native::ops::{LaneOps, WideLaneOps};

macro_rules! k_mask {
    ($lanes:expr) => {
        #[inline(always)]
        fn mask_not(mask: u64) -> u64 {
            !mask & lane_bits($lanes)
        }

        #[inline(always)]
        fn mask_all(mask: u64) -> bool {
            mask == lane_bits($lanes)
        }

        #[inline(always)]
        fn bitmask(mask: u64) -> u64 {
            mask
        }

        #[inline(always)]
        fn from_bitmask(bits: u64) -> u64 {
            bits
        }
    };
}

#[inline(always)]
fn zip(low_lanes: __m512i, high_lanes: __m512i) -> (__m512i, __m512i) {
    unsafe {
        (
            _mm512_permutex2var_epi64(low_lanes, qwords([0, 1, 8, 9, 2, 3, 10, 11]), high_lanes),
            _mm512_permutex2var_epi64(low_lanes, qwords([4, 5, 12, 13, 6, 7, 14, 15]), high_lanes),
        )
    }
}

#[inline(always)]
fn count_bytes(a: __m512i) -> __m512i {
    unsafe {
        let nibbles = _mm512_set1_epi8(0x0F);
        let table = _mm512_broadcast_i32x4(_mm_setr_epi8(
            0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4,
        ));
        let low_nibbles = _mm512_and_si512(a, nibbles);
        let high_nibbles = _mm512_and_si512(_mm512_srli_epi16::<4>(a), nibbles);
        _mm512_add_epi8(
            _mm512_shuffle_epi8(table, low_nibbles),
            _mm512_shuffle_epi8(table, high_nibbles),
        )
    }
}

#[inline(always)]
fn count_words(a: __m512i) -> __m512i {
    unsafe {
        let bytes = count_bytes(a);
        _mm512_add_epi16(
            _mm512_and_si512(bytes, _mm512_set1_epi16(0xFF)),
            _mm512_srli_epi16::<8>(bytes),
        )
    }
}

#[inline(always)]
fn word_order(first: i16) -> __m512i {
    from_lanes::<__m512i, i16, 32>(std::array::from_fn(|lane| first + 2 * lane as i16))
}

#[inline(always)]
fn dword_order(first: i32) -> __m512i {
    from_lanes::<__m512i, i32, 16>(std::array::from_fn(|lane| first + 2 * lane as i32))
}

#[inline(always)]
pub(super) fn byte_shift_right_arithmetic(a: __m512i, counts: __m512i) -> __m512i {
    unsafe {
        let low_bytes = _mm512_set1_epi16(0xFF);
        let even = _mm512_srli_epi16::<8>(_mm512_srav_epi16(
            _mm512_slli_epi16::<8>(a),
            _mm512_and_si512(counts, low_bytes),
        ));
        let odd = _mm512_srav_epi16(a, _mm512_srli_epi16::<8>(counts));
        _mm512_or_si512(even, _mm512_andnot_si512(low_bytes, odd))
    }
}

impl LaneOps<__m512i> for u8 {
    k_mask!(64);

    #[inline(always)]
    fn select<S: Simd>(mask: u64, if_true: __m512i, if_false: __m512i) -> __m512i {
        unsafe { _mm512_mask_blend_epi8(mask, if_false, if_true) }
    }

    #[inline(always)]
    fn splat(value: u8) -> __m512i {
        unsafe { _mm512_set1_epi8(value as i8) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m512i, b: __m512i) -> u64 {
        unsafe { _mm512_cmpeq_epi8_mask(a, b) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_add_epi8(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_sub_epi8(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe {
            let even = _mm512_mullo_epi16(a, b);
            let odd = _mm512_mullo_epi16(_mm512_srli_epi16::<8>(a), _mm512_srli_epi16::<8>(b));
            _mm512_or_si512(
                _mm512_and_si512(even, _mm512_set1_epi16(0xFF)),
                _mm512_slli_epi16::<8>(odd),
            )
        }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        let byte_mask = 0xFFu32.checked_shl(shift).unwrap_or(0) as u8;
        unsafe {
            _mm512_and_si512(
                _mm512_sll_epi16(a, shift_count(shift)),
                _mm512_set1_epi8(byte_mask as i8),
            )
        }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        let byte_mask = 0xFFu32.checked_shr(shift).unwrap_or(0) as u8;
        unsafe {
            _mm512_and_si512(
                _mm512_srl_epi16(a, shift_count(shift)),
                _mm512_set1_epi8(byte_mask as i8),
            )
        }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe {
            let low_bytes = _mm512_set1_epi16(0xFF);
            let even = _mm512_and_si512(
                _mm512_sllv_epi16(a, _mm512_and_si512(counts, low_bytes)),
                low_bytes,
            );
            let odd = _mm512_sllv_epi16(
                _mm512_andnot_si512(low_bytes, a),
                _mm512_srli_epi16::<8>(counts),
            );
            _mm512_or_si512(even, _mm512_andnot_si512(low_bytes, odd))
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe {
            let low_bytes = _mm512_set1_epi16(0xFF);
            let even = _mm512_srlv_epi16(
                _mm512_and_si512(a, low_bytes),
                _mm512_and_si512(counts, low_bytes),
            );
            let odd = _mm512_srlv_epi16(a, _mm512_srli_epi16::<8>(counts));
            _mm512_or_si512(even, _mm512_andnot_si512(low_bytes, odd))
        }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m512i, b: __m512i) -> (__m512i, __m512i) {
        unsafe { zip(_mm512_unpacklo_epi8(a, b), _mm512_unpackhi_epi8(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m512i, b: __m512i) -> (__m512i, __m512i) {
        unsafe {
            let low_bytes = _mm512_set1_epi16(0xFF);
            let order = qwords([0, 2, 4, 6, 1, 3, 5, 7]);
            let even = _mm512_packus_epi16(
                _mm512_and_si512(a, low_bytes),
                _mm512_and_si512(b, low_bytes),
            );
            let odd = _mm512_packus_epi16(_mm512_srli_epi16::<8>(a), _mm512_srli_epi16::<8>(b));
            (
                _mm512_permutexvar_epi64(order, even),
                _mm512_permutexvar_epi64(order, odd),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m512i) -> __m512i {
        unsafe {
            let order = _mm512_broadcast_i32x4(_mm_setr_epi8(
                15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0,
            ));
            let reversed = _mm512_shuffle_epi8(a, order);
            _mm512_shuffle_i64x2::<0x1B>(reversed, reversed)
        }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u8, len: usize) -> __m512i {
        unsafe { _mm512_maskz_loadu_epi8(lane_bits(len), ptr.cast()) }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m512i, ptr: *mut u8, len: usize) {
        unsafe { _mm512_mask_storeu_epi8(ptr.cast(), lane_bits(len), value) }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m512i) -> u8 {
        let halves = unsafe { _mm256_add_epi8(low(a), high(a)) };
        <u8 as LaneOps<__m256i>>::reduce_add::<S>(halves)
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m512i) -> __m512i {
        unsafe {
            let a = _mm512_add_epi8(a, _mm512_bslli_epi128::<1>(a));
            let a = _mm512_add_epi8(a, _mm512_bslli_epi128::<2>(a));
            let a = _mm512_add_epi8(a, _mm512_bslli_epi128::<4>(a));
            let a = _mm512_add_epi8(a, _mm512_bslli_epi128::<8>(a));
            carry_blocks(
                a,
                |x| _mm512_shuffle_epi8(x, _mm512_set1_epi8(15)),
                |x, y| _mm512_add_epi8(x, y),
            )
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m512i) -> __m512i {
        count_bytes(a)
    }
}

impl LaneOps<__m512i> for u16 {
    k_mask!(32);

    #[inline(always)]
    fn select<S: Simd>(mask: u64, if_true: __m512i, if_false: __m512i) -> __m512i {
        unsafe { _mm512_mask_blend_epi16(mask as u32, if_false, if_true) }
    }

    #[inline(always)]
    fn splat(value: u16) -> __m512i {
        unsafe { _mm512_set1_epi16(value as i16) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m512i, b: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmpeq_epi16_mask(a, b)) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_add_epi16(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_sub_epi16(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_mullo_epi16(a, b) }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        unsafe { _mm512_sll_epi16(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        unsafe { _mm512_srl_epi16(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe { _mm512_sllv_epi16(a, counts) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe { _mm512_srlv_epi16(a, counts) }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m512i, b: __m512i) -> (__m512i, __m512i) {
        unsafe { zip(_mm512_unpacklo_epi16(a, b), _mm512_unpackhi_epi16(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m512i, b: __m512i) -> (__m512i, __m512i) {
        unsafe {
            (
                _mm512_permutex2var_epi16(a, word_order(0), b),
                _mm512_permutex2var_epi16(a, word_order(1), b),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m512i) -> __m512i {
        let order = from_lanes::<__m512i, i16, 32>(std::array::from_fn(|lane| 31 - lane as i16));
        unsafe { _mm512_permutexvar_epi16(order, a) }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u16, len: usize) -> __m512i {
        unsafe { _mm512_maskz_loadu_epi16(lane_bits(len) as u32, ptr.cast()) }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m512i, ptr: *mut u16, len: usize) {
        unsafe { _mm512_mask_storeu_epi16(ptr.cast(), lane_bits(len) as u32, value) }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m512i) -> u16 {
        let halves = unsafe { _mm256_add_epi16(low(a), high(a)) };
        <u16 as LaneOps<__m256i>>::reduce_add::<S>(halves)
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m512i) -> __m512i {
        unsafe {
            let a = _mm512_add_epi16(a, _mm512_bslli_epi128::<2>(a));
            let a = _mm512_add_epi16(a, _mm512_bslli_epi128::<4>(a));
            let a = _mm512_add_epi16(a, _mm512_bslli_epi128::<8>(a));
            carry_blocks(
                a,
                |x| _mm512_shuffle_epi8(x, _mm512_set1_epi16(0x0F0E)),
                |x, y| _mm512_add_epi16(x, y),
            )
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m512i) -> __m512i {
        count_words(a)
    }
}

impl LaneOps<__m512i> for u32 {
    k_mask!(16);

    #[inline(always)]
    fn select<S: Simd>(mask: u64, if_true: __m512i, if_false: __m512i) -> __m512i {
        unsafe { _mm512_mask_blend_epi32(mask as u16, if_false, if_true) }
    }

    #[inline(always)]
    fn splat(value: u32) -> __m512i {
        unsafe { _mm512_set1_epi32(value as i32) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m512i, b: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmpeq_epi32_mask(a, b)) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_add_epi32(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_sub_epi32(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_mullo_epi32(a, b) }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        unsafe { _mm512_sll_epi32(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        unsafe { _mm512_srl_epi32(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe { _mm512_sllv_epi32(a, counts) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe { _mm512_srlv_epi32(a, counts) }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m512i, b: __m512i) -> (__m512i, __m512i) {
        unsafe { zip(_mm512_unpacklo_epi32(a, b), _mm512_unpackhi_epi32(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m512i, b: __m512i) -> (__m512i, __m512i) {
        unsafe {
            (
                _mm512_permutex2var_epi32(a, dword_order(0), b),
                _mm512_permutex2var_epi32(a, dword_order(1), b),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m512i) -> __m512i {
        let order = from_lanes::<__m512i, i32, 16>(std::array::from_fn(|lane| 15 - lane as i32));
        unsafe { _mm512_permutexvar_epi32(order, a) }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u32, len: usize) -> __m512i {
        unsafe { _mm512_maskz_loadu_epi32(lane_bits(len) as u16, ptr.cast()) }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m512i, ptr: *mut u32, len: usize) {
        unsafe { _mm512_mask_storeu_epi32(ptr.cast(), lane_bits(len) as u16, value) }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m512i) -> u32 {
        let halves = unsafe { _mm256_add_epi32(low(a), high(a)) };
        <u32 as LaneOps<__m256i>>::reduce_add::<S>(halves)
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m512i) -> __m512i {
        unsafe {
            let a = _mm512_add_epi32(a, _mm512_bslli_epi128::<4>(a));
            let a = _mm512_add_epi32(a, _mm512_bslli_epi128::<8>(a));
            carry_blocks(
                a,
                |x| _mm512_shuffle_epi32::<0xFF>(x),
                |x, y| _mm512_add_epi32(x, y),
            )
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m512i) -> __m512i {
        unsafe {
            let words = count_words(a);
            _mm512_add_epi32(
                _mm512_and_si512(words, _mm512_set1_epi32(0xFFFF)),
                _mm512_srli_epi32::<16>(words),
            )
        }
    }
}

impl LaneOps<__m512i> for u64 {
    k_mask!(8);

    #[inline(always)]
    fn select<S: Simd>(mask: u64, if_true: __m512i, if_false: __m512i) -> __m512i {
        unsafe { _mm512_mask_blend_epi64(mask as u8, if_false, if_true) }
    }

    #[inline(always)]
    fn splat(value: u64) -> __m512i {
        unsafe { _mm512_set1_epi64(value as i64) }
    }

    #[inline(always)]
    fn equal<S: Simd>(a: __m512i, b: __m512i) -> u64 {
        unsafe { u64::from(_mm512_cmpeq_epi64_mask(a, b)) }
    }

    #[inline(always)]
    fn add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_add_epi64(a, b) }
    }

    #[inline(always)]
    fn sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_sub_epi64(a, b) }
    }

    #[inline(always)]
    fn mul<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_mullo_epi64(a, b) }
    }

    #[inline(always)]
    fn shl<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        unsafe { _mm512_sll_epi64(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        unsafe { _mm512_srl_epi64(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shl_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe { _mm512_sllv_epi64(a, counts) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe { _mm512_srlv_epi64(a, counts) }
    }

    #[inline(always)]
    fn interleave<S: Simd>(a: __m512i, b: __m512i) -> (__m512i, __m512i) {
        unsafe { zip(_mm512_unpacklo_epi64(a, b), _mm512_unpackhi_epi64(a, b)) }
    }

    #[inline(always)]
    fn deinterleave<S: Simd>(a: __m512i, b: __m512i) -> (__m512i, __m512i) {
        unsafe {
            (
                _mm512_permutex2var_epi64(a, qwords([0, 2, 4, 6, 8, 10, 12, 14]), b),
                _mm512_permutex2var_epi64(a, qwords([1, 3, 5, 7, 9, 11, 13, 15]), b),
            )
        }
    }

    #[inline(always)]
    fn reverse<S: Simd>(a: __m512i) -> __m512i {
        unsafe { _mm512_permutexvar_epi64(qwords([7, 6, 5, 4, 3, 2, 1, 0]), a) }
    }

    #[inline(always)]
    unsafe fn load_partial<S: Simd>(ptr: *const u64, len: usize) -> __m512i {
        unsafe { _mm512_maskz_loadu_epi64(lane_bits(len) as u8, ptr.cast()) }
    }

    #[inline(always)]
    unsafe fn store_partial<S: Simd>(value: __m512i, ptr: *mut u64, len: usize) {
        unsafe { _mm512_mask_storeu_epi64(ptr.cast(), lane_bits(len) as u8, value) }
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: __m512i) -> u64 {
        let halves = unsafe { _mm256_add_epi64(low(a), high(a)) };
        <u64 as LaneOps<__m256i>>::reduce_add::<S>(halves)
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: __m512i) -> __m512i {
        unsafe {
            let a = _mm512_add_epi64(a, _mm512_bslli_epi128::<8>(a));
            carry_blocks(
                a,
                |x| _mm512_shuffle_epi32::<0xEE>(x),
                |x, y| _mm512_add_epi64(x, y),
            )
        }
    }

    #[inline(always)]
    fn count_ones<S: Simd>(a: __m512i) -> __m512i {
        unsafe { _mm512_sad_epu8(count_bytes(a), _mm512_setzero_si512()) }
    }
}

impl WideLaneOps<__m512i> for u32 {
    #[inline(always)]
    unsafe fn gather<S: Simd>(base: *const u32, indices: __m512i) -> __m512i {
        unsafe { _mm512_i32gather_epi32::<4>(indices, base.cast()) }
    }

    #[inline(always)]
    fn compress<S: Simd>(a: __m512i, mask: u64) -> __m512i {
        unsafe { _mm512_maskz_compress_epi32(mask as u16, a) }
    }
}

impl WideLaneOps<__m512i> for u64 {
    #[inline(always)]
    unsafe fn gather<S: Simd>(base: *const u64, indices: __m512i) -> __m512i {
        unsafe { _mm512_i64gather_epi64::<8>(indices, base.cast()) }
    }

    #[inline(always)]
    fn compress<S: Simd>(a: __m512i, mask: u64) -> __m512i {
        unsafe { _mm512_maskz_compress_epi64(mask as u8, a) }
    }
}
