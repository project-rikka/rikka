use std::arch::x86_64::*;

use super::lanes::byte_shift_right_arithmetic;
use super::{blend, from_ps, not, ps, shift_count};
use crate::Simd;
use crate::native::map_lanes;
use crate::native::ops::{
    ElemOps, IntOps, LaneOps, NarrowOps, NarrowUnsignedOps, NegOps, SignedOps, SwizzleOps, WidenOps,
};

#[inline(always)]
fn fold(lane_bytes: usize, a: __m128i, op: impl Fn(__m128i, __m128i) -> __m128i) -> __m128i {
    unsafe {
        let mut a = op(a, _mm_srli_si128::<8>(a));
        if lane_bytes <= 4 {
            a = op(a, _mm_srli_si128::<4>(a));
        }
        if lane_bytes <= 2 {
            a = op(a, _mm_srli_si128::<2>(a));
        }
        if lane_bytes <= 1 {
            a = op(a, _mm_srli_si128::<1>(a));
        }
        a
    }
}

#[inline(always)]
fn flip8(a: __m128i) -> __m128i {
    unsafe { _mm_xor_si128(a, _mm_set1_epi8(i8::MIN)) }
}

#[inline(always)]
fn flip16(a: __m128i) -> __m128i {
    unsafe { _mm_xor_si128(a, _mm_set1_epi16(i16::MIN)) }
}

#[inline(always)]
fn flip32(a: __m128i) -> __m128i {
    unsafe { _mm_xor_si128(a, _mm_set1_epi32(i32::MIN)) }
}

#[inline(always)]
fn flip64(a: __m128i) -> __m128i {
    unsafe { _mm_xor_si128(a, _mm_set1_epi64x(i64::MIN)) }
}

#[inline(always)]
fn sign32(a: __m128i) -> __m128i {
    unsafe { _mm_srai_epi32::<31>(a) }
}

#[inline(always)]
fn sign64(a: __m128i) -> __m128i {
    unsafe { _mm_shuffle_epi32::<0xF5>(_mm_srai_epi32::<31>(a)) }
}

#[inline(always)]
fn gt64<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
    unsafe {
        if S::SSE42 {
            _mm_cmpgt_epi64(a, b)
        } else {
            let difference = _mm_sub_epi64(b, a);
            let high_equal = _mm_cmpeq_epi32(a, b);
            let high_greater = _mm_cmpgt_epi32(a, b);
            let greater = _mm_or_si128(high_greater, _mm_and_si128(high_equal, difference));
            _mm_shuffle_epi32::<0xF5>(_mm_srai_epi32::<31>(greater))
        }
    }
}

#[inline(always)]
fn min_i32<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
    unsafe {
        if S::SSE42 {
            _mm_min_epi32(a, b)
        } else {
            blend::<S>(_mm_cmpgt_epi32(a, b), b, a)
        }
    }
}

#[inline(always)]
fn max_i32<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
    unsafe {
        if S::SSE42 {
            _mm_max_epi32(a, b)
        } else {
            blend::<S>(_mm_cmpgt_epi32(a, b), a, b)
        }
    }
}

#[inline(always)]
fn lt_u32(a: __m128i, b: __m128i) -> __m128i {
    unsafe { _mm_cmpgt_epi32(flip32(b), flip32(a)) }
}

#[inline(always)]
fn min_u32<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
    unsafe {
        if S::SSE42 {
            _mm_min_epu32(a, b)
        } else {
            blend::<S>(lt_u32(a, b), a, b)
        }
    }
}

#[inline(always)]
fn max_u32<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
    unsafe {
        if S::SSE42 {
            _mm_max_epu32(a, b)
        } else {
            blend::<S>(lt_u32(a, b), b, a)
        }
    }
}

#[inline(always)]
fn lt_u64<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
    unsafe {
        if S::AVX512 {
            _mm_movm_epi64(_mm_cmplt_epu64_mask(a, b))
        } else {
            gt64::<S>(flip64(b), flip64(a))
        }
    }
}

#[inline(always)]
fn min_u16<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
    unsafe {
        if S::SSE42 {
            _mm_min_epu16(a, b)
        } else {
            _mm_sub_epi16(a, _mm_subs_epu16(a, b))
        }
    }
}

#[inline(always)]
fn pack_u32_to_u16<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
    unsafe {
        if S::SSE42 {
            _mm_packus_epi32(low, high)
        } else {
            let bias = _mm_set1_epi32(0x8000);
            _mm_add_epi16(
                _mm_packs_epi32(_mm_sub_epi32(low, bias), _mm_sub_epi32(high, bias)),
                _mm_set1_epi16(i16::MIN),
            )
        }
    }
}

#[inline(always)]
fn low_halves(low: __m128i, high: __m128i) -> __m128i {
    unsafe { from_ps(_mm_shuffle_ps::<0x88>(ps(low), ps(high))) }
}

macro_rules! reductions {
    ($element:ty, $bytes:expr, $extract:ident) => {
        #[inline(always)]
        fn reduce_min<S: Simd>(a: __m128i) -> $element {
            unsafe {
                $extract(fold($bytes, a, <Self as ElemOps<__m128i>>::minimum::<S>)) as $element
            }
        }

        #[inline(always)]
        fn reduce_max<S: Simd>(a: __m128i) -> $element {
            unsafe {
                $extract(fold($bytes, a, <Self as ElemOps<__m128i>>::maximum::<S>)) as $element
            }
        }
    };
}

impl ElemOps<__m128i> for i8 {
    reductions!(i8, 1, _mm_cvtsi128_si32);

    #[inline(always)]
    fn less<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_cmpgt_epi8(b, a) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { not(_mm_cmpgt_epi8(a, b)) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_min_epi8(a, b)
            } else {
                flip8(_mm_min_epu8(flip8(a), flip8(b)))
            }
        }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_max_epi8(a, b)
            } else {
                flip8(_mm_max_epu8(flip8(a), flip8(b)))
            }
        }
    }
}

impl ElemOps<__m128i> for u8 {
    reductions!(u8, 1, _mm_cvtsi128_si32);

    #[inline(always)]
    fn less<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_cmpgt_epi8(flip8(b), flip8(a)) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_cmpeq_epi8(_mm_subs_epu8(a, b), _mm_setzero_si128()) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_min_epu8(a, b) }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_max_epu8(a, b) }
    }
}

impl ElemOps<__m128i> for i16 {
    reductions!(i16, 2, _mm_cvtsi128_si32);

    #[inline(always)]
    fn less<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_cmpgt_epi16(b, a) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { not(_mm_cmpgt_epi16(a, b)) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_min_epi16(a, b) }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_max_epi16(a, b) }
    }
}

impl ElemOps<__m128i> for u16 {
    reductions!(u16, 2, _mm_cvtsi128_si32);

    #[inline(always)]
    fn less<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_cmpgt_epi16(flip16(b), flip16(a)) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_cmpeq_epi16(_mm_subs_epu16(a, b), _mm_setzero_si128()) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        min_u16::<S>(a, b)
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_max_epu16(a, b)
            } else {
                _mm_add_epi16(b, _mm_subs_epu16(a, b))
            }
        }
    }
}

impl ElemOps<__m128i> for i32 {
    reductions!(i32, 4, _mm_cvtsi128_si32);

    #[inline(always)]
    fn less<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_cmpgt_epi32(b, a) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { not(_mm_cmpgt_epi32(a, b)) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        min_i32::<S>(a, b)
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        max_i32::<S>(a, b)
    }
}

impl ElemOps<__m128i> for u32 {
    reductions!(u32, 4, _mm_cvtsi128_si32);

    #[inline(always)]
    fn less<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        lt_u32(a, b)
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_cmpeq_epi32(_mm_max_epu32(a, b), b)
            } else {
                not(lt_u32(b, a))
            }
        }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        min_u32::<S>(a, b)
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        max_u32::<S>(a, b)
    }
}

impl ElemOps<__m128i> for i64 {
    reductions!(i64, 8, _mm_cvtsi128_si64);

    #[inline(always)]
    fn less<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        gt64::<S>(b, a)
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        not(gt64::<S>(a, b))
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_min_epi64(a, b)
            } else {
                blend::<S>(gt64::<S>(a, b), b, a)
            }
        }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_max_epi64(a, b)
            } else {
                blend::<S>(gt64::<S>(a, b), a, b)
            }
        }
    }
}

impl ElemOps<__m128i> for u64 {
    reductions!(u64, 8, _mm_cvtsi128_si64);

    #[inline(always)]
    fn less<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        lt_u64::<S>(a, b)
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        not(lt_u64::<S>(b, a))
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_min_epu64(a, b)
            } else {
                blend::<S>(lt_u64::<S>(a, b), a, b)
            }
        }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_max_epu64(a, b)
            } else {
                blend::<S>(lt_u64::<S>(a, b), b, a)
            }
        }
    }
}

impl IntOps<__m128i> for i8 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_adds_epi8(a, b) }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_subs_epi8(a, b) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe {
            let shift = shift.min(7);
            let logical = <u8 as LaneOps<__m128i>>::shr::<S>(a, shift);
            let sign = _mm_set1_epi8((0x80u8 >> shift) as i8);
            _mm_sub_epi8(_mm_xor_si128(logical, sign), sign)
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        byte_shift_right_arithmetic::<S>(a, counts)
    }
}

impl IntOps<__m128i> for u8 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_adds_epu8(a, b) }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_subs_epu8(a, b) }
    }
}

impl IntOps<__m128i> for i16 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_adds_epi16(a, b) }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_subs_epi16(a, b) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe { _mm_sra_epi16(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_srav_epi16(a, counts)
            } else {
                map_lanes::<_, i16, 8>(a, counts, |x, n| x >> (n as u16).min(15))
            }
        }
    }
}

impl IntOps<__m128i> for u16 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_adds_epu16(a, b) }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_subs_epu16(a, b) }
    }
}

impl IntOps<__m128i> for i32 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            let sum = _mm_add_epi32(a, b);
            let overflow = sign32(_mm_and_si128(_mm_xor_si128(a, sum), _mm_xor_si128(b, sum)));
            let saturated = _mm_xor_si128(sign32(a), _mm_set1_epi32(i32::MAX));
            blend::<S>(overflow, saturated, sum)
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            let difference = _mm_sub_epi32(a, b);
            let overflow = sign32(_mm_and_si128(
                _mm_xor_si128(a, b),
                _mm_xor_si128(a, difference),
            ));
            let saturated = _mm_xor_si128(sign32(a), _mm_set1_epi32(i32::MAX));
            blend::<S>(overflow, saturated, difference)
        }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe { _mm_sra_epi32(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        unsafe {
            if S::AVX2 {
                _mm_srav_epi32(a, counts)
            } else {
                map_lanes::<_, i32, 4>(a, counts, |x, n| x >> (n as u32).min(31))
            }
        }
    }
}

impl IntOps<__m128i> for u32 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            let sum = _mm_add_epi32(a, b);
            _mm_or_si128(sum, lt_u32(sum, a))
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_andnot_si128(lt_u32(a, b), _mm_sub_epi32(a, b)) }
    }
}

impl IntOps<__m128i> for i64 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            let sum = _mm_add_epi64(a, b);
            let overflow = sign64(_mm_and_si128(_mm_xor_si128(a, sum), _mm_xor_si128(b, sum)));
            let saturated = _mm_xor_si128(sign64(a), _mm_set1_epi64x(i64::MAX));
            blend::<S>(overflow, saturated, sum)
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            let difference = _mm_sub_epi64(a, b);
            let overflow = sign64(_mm_and_si128(
                _mm_xor_si128(a, b),
                _mm_xor_si128(a, difference),
            ));
            let saturated = _mm_xor_si128(sign64(a), _mm_set1_epi64x(i64::MAX));
            blend::<S>(overflow, saturated, difference)
        }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m128i, shift: u32) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_sra_epi64(a, shift_count(shift))
            } else {
                let shift = shift.min(63);
                let logical = _mm_srl_epi64(a, shift_count(shift));
                let sign = _mm_set1_epi64x(((1u64 << 63) >> shift) as i64);
                _mm_sub_epi64(_mm_xor_si128(logical, sign), sign)
            }
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m128i, counts: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_srav_epi64(a, counts)
            } else if S::AVX2 {
                let negative = sign64(a);
                _mm_xor_si128(_mm_srlv_epi64(_mm_xor_si128(a, negative), counts), negative)
            } else {
                map_lanes::<_, i64, 2>(a, counts, |x, n| x >> (n as u64).min(63))
            }
        }
    }
}

impl IntOps<__m128i> for u64 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe {
            let sum = _mm_add_epi64(a, b);
            _mm_or_si128(sum, lt_u64::<S>(sum, a))
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m128i, b: __m128i) -> __m128i {
        unsafe { _mm_andnot_si128(lt_u64::<S>(a, b), _mm_sub_epi64(a, b)) }
    }
}

impl NegOps<__m128i> for i8 {}
impl NegOps<__m128i> for i16 {}
impl NegOps<__m128i> for i32 {}
impl NegOps<__m128i> for i64 {}

impl SignedOps<__m128i> for i8 {
    #[inline(always)]
    fn abs<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_abs_epi8(a)
            } else {
                let sign = _mm_cmpgt_epi8(_mm_setzero_si128(), a);
                _mm_sub_epi8(_mm_xor_si128(a, sign), sign)
            }
        }
    }
}

impl SignedOps<__m128i> for i16 {
    #[inline(always)]
    fn abs<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_abs_epi16(a)
            } else {
                _mm_max_epi16(a, _mm_sub_epi16(_mm_setzero_si128(), a))
            }
        }
    }
}

impl SignedOps<__m128i> for i32 {
    #[inline(always)]
    fn abs<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_abs_epi32(a)
            } else {
                let sign = sign32(a);
                _mm_sub_epi32(_mm_xor_si128(a, sign), sign)
            }
        }
    }
}

impl SignedOps<__m128i> for i64 {
    #[inline(always)]
    fn abs<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_abs_epi64(a)
            } else {
                let sign = sign64(a);
                _mm_sub_epi64(_mm_xor_si128(a, sign), sign)
            }
        }
    }
}

impl WidenOps<__m128i> for i8 {
    type Wide = i16;

    #[inline(always)]
    fn widen_low<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_cvtepi8_epi16(a)
            } else {
                _mm_unpacklo_epi8(a, _mm_cmpgt_epi8(_mm_setzero_si128(), a))
            }
        }
    }

    #[inline(always)]
    fn widen_high<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_cvtepi8_epi16(_mm_srli_si128::<8>(a))
            } else {
                _mm_unpackhi_epi8(a, _mm_cmpgt_epi8(_mm_setzero_si128(), a))
            }
        }
    }
}

impl WidenOps<__m128i> for u8 {
    type Wide = u16;

    #[inline(always)]
    fn widen_low<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_unpacklo_epi8(a, _mm_setzero_si128()) }
    }

    #[inline(always)]
    fn widen_high<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_unpackhi_epi8(a, _mm_setzero_si128()) }
    }
}

impl WidenOps<__m128i> for i16 {
    type Wide = i32;

    #[inline(always)]
    fn widen_low<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_cvtepi16_epi32(a)
            } else {
                _mm_srai_epi32::<16>(_mm_unpacklo_epi16(a, a))
            }
        }
    }

    #[inline(always)]
    fn widen_high<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_cvtepi16_epi32(_mm_srli_si128::<8>(a))
            } else {
                _mm_srai_epi32::<16>(_mm_unpackhi_epi16(a, a))
            }
        }
    }
}

impl WidenOps<__m128i> for u16 {
    type Wide = u32;

    #[inline(always)]
    fn widen_low<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_unpacklo_epi16(a, _mm_setzero_si128()) }
    }

    #[inline(always)]
    fn widen_high<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_unpackhi_epi16(a, _mm_setzero_si128()) }
    }
}

impl WidenOps<__m128i> for i32 {
    type Wide = i64;

    #[inline(always)]
    fn widen_low<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_cvtepi32_epi64(a)
            } else {
                _mm_unpacklo_epi32(a, sign32(a))
            }
        }
    }

    #[inline(always)]
    fn widen_high<S: Simd>(a: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_cvtepi32_epi64(_mm_srli_si128::<8>(a))
            } else {
                _mm_unpackhi_epi32(a, sign32(a))
            }
        }
    }
}

impl WidenOps<__m128i> for u32 {
    type Wide = u64;

    #[inline(always)]
    fn widen_low<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_unpacklo_epi32(a, _mm_setzero_si128()) }
    }

    #[inline(always)]
    fn widen_high<S: Simd>(a: __m128i) -> __m128i {
        unsafe { _mm_unpackhi_epi32(a, _mm_setzero_si128()) }
    }
}

impl NarrowOps<__m128i> for i16 {
    type Narrow = i8;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
        unsafe { _mm_packs_epi16(low, high) }
    }
}

impl NarrowOps<__m128i> for u16 {
    type Narrow = u8;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
        unsafe {
            let cap = _mm_set1_epi16(0xFF);
            _mm_packus_epi16(min_u16::<S>(low, cap), min_u16::<S>(high, cap))
        }
    }
}

impl NarrowOps<__m128i> for i32 {
    type Narrow = i16;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
        unsafe { _mm_packs_epi32(low, high) }
    }
}

impl NarrowOps<__m128i> for u32 {
    type Narrow = u16;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
        unsafe {
            let cap = _mm_set1_epi32(0xFFFF);
            pack_u32_to_u16::<S>(min_u32::<S>(low, cap), min_u32::<S>(high, cap))
        }
    }
}

impl NarrowOps<__m128i> for i64 {
    type Narrow = i32;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_unpacklo_epi64(_mm_cvtsepi64_epi32(low), _mm_cvtsepi64_epi32(high))
            } else {
                let lower = _mm_set1_epi64x(i64::from(i32::MIN));
                let upper = _mm_set1_epi64x(i64::from(i32::MAX));
                let clamp = |x| {
                    <Self as ElemOps<__m128i>>::minimum::<S>(
                        <Self as ElemOps<__m128i>>::maximum::<S>(x, lower),
                        upper,
                    )
                };
                low_halves(clamp(low), clamp(high))
            }
        }
    }
}

impl NarrowOps<__m128i> for u64 {
    type Narrow = u32;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
        unsafe {
            if S::AVX512 {
                _mm_unpacklo_epi64(_mm_cvtusepi64_epi32(low), _mm_cvtusepi64_epi32(high))
            } else {
                let upper = _mm_set1_epi64x(i64::from(u32::MAX));
                let clamp = |x| <Self as ElemOps<__m128i>>::minimum::<S>(x, upper);
                low_halves(clamp(low), clamp(high))
            }
        }
    }
}

impl NarrowUnsignedOps<__m128i> for i16 {
    type NarrowUnsigned = u8;

    #[inline(always)]
    fn narrow_unsigned<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
        unsafe { _mm_packus_epi16(low, high) }
    }
}

impl NarrowUnsignedOps<__m128i> for i32 {
    type NarrowUnsigned = u16;

    #[inline(always)]
    fn narrow_unsigned<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_packus_epi32(low, high)
            } else {
                let lower = _mm_setzero_si128();
                let upper = _mm_set1_epi32(0xFFFF);
                let clamp = |x| min_i32::<S>(max_i32::<S>(x, lower), upper);
                pack_u32_to_u16::<S>(clamp(low), clamp(high))
            }
        }
    }
}

impl NarrowUnsignedOps<__m128i> for i64 {
    type NarrowUnsigned = u32;

    #[inline(always)]
    fn narrow_unsigned<S: Simd>(low: __m128i, high: __m128i) -> __m128i {
        unsafe {
            let lower = _mm_setzero_si128();
            if S::AVX512 {
                _mm_unpacklo_epi64(
                    _mm_cvtusepi64_epi32(_mm_max_epi64(low, lower)),
                    _mm_cvtusepi64_epi32(_mm_max_epi64(high, lower)),
                )
            } else {
                let upper = _mm_set1_epi64x(i64::from(u32::MAX));
                let clamp = |x| {
                    <Self as ElemOps<__m128i>>::minimum::<S>(
                        <Self as ElemOps<__m128i>>::maximum::<S>(x, lower),
                        upper,
                    )
                };
                low_halves(clamp(low), clamp(high))
            }
        }
    }
}

impl SwizzleOps<__m128i> for u8 {
    #[inline(always)]
    fn swizzle<S: Simd>(table: __m128i, indices: __m128i) -> __m128i {
        unsafe {
            if S::SSE42 {
                _mm_shuffle_epi8(table, _mm_adds_epu8(indices, _mm_set1_epi8(0x70)))
            } else {
                let lanes: [u8; 16] = crate::native::to_lanes(table);
                let indices: [u8; 16] = crate::native::to_lanes(indices);
                crate::native::from_lanes(
                    indices.map(|index| lanes.get(usize::from(index)).copied().unwrap_or(0)),
                )
            }
        }
    }
}
