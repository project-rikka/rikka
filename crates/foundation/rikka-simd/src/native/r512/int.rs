use std::arch::x86_64::*;

use super::lanes::byte_shift_right_arithmetic;
use super::{high, join, low, shift_count};
use crate::Simd;
use crate::native::ops::{
    ElemOps, IntOps, LaneOps, NarrowOps, NarrowUnsignedOps, NegOps, SignedOps, SwizzleOps, WidenOps,
};

macro_rules! reductions {
    ($element:ty) => {
        #[inline(always)]
        fn reduce_min<S: Simd>(a: __m512i) -> $element {
            let halves = <$element as ElemOps<__m256i>>::minimum::<S>(low(a), high(a));
            <$element as ElemOps<__m256i>>::reduce_min::<S>(halves)
        }

        #[inline(always)]
        fn reduce_max<S: Simd>(a: __m512i) -> $element {
            let halves = <$element as ElemOps<__m256i>>::maximum::<S>(low(a), high(a));
            <$element as ElemOps<__m256i>>::reduce_max::<S>(halves)
        }
    };
}

macro_rules! compares {
    ($element:ty, $less:ident, $less_equal:ident, $min:ident, $max:ident) => {
        impl ElemOps<__m512i> for $element {
            reductions!($element);

            #[inline(always)]
            fn less<S: Simd>(a: __m512i, b: __m512i) -> u64 {
                unsafe { u64::from($less(a, b)) }
            }

            #[inline(always)]
            fn less_equal<S: Simd>(a: __m512i, b: __m512i) -> u64 {
                unsafe { u64::from($less_equal(a, b)) }
            }

            #[inline(always)]
            fn minimum<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
                unsafe { $min(a, b) }
            }

            #[inline(always)]
            fn maximum<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
                unsafe { $max(a, b) }
            }
        }
    };
}

compares!(
    i8,
    _mm512_cmplt_epi8_mask,
    _mm512_cmple_epi8_mask,
    _mm512_min_epi8,
    _mm512_max_epi8
);
compares!(
    u8,
    _mm512_cmplt_epu8_mask,
    _mm512_cmple_epu8_mask,
    _mm512_min_epu8,
    _mm512_max_epu8
);
compares!(
    i16,
    _mm512_cmplt_epi16_mask,
    _mm512_cmple_epi16_mask,
    _mm512_min_epi16,
    _mm512_max_epi16
);
compares!(
    u16,
    _mm512_cmplt_epu16_mask,
    _mm512_cmple_epu16_mask,
    _mm512_min_epu16,
    _mm512_max_epu16
);
compares!(
    i32,
    _mm512_cmplt_epi32_mask,
    _mm512_cmple_epi32_mask,
    _mm512_min_epi32,
    _mm512_max_epi32
);
compares!(
    u32,
    _mm512_cmplt_epu32_mask,
    _mm512_cmple_epu32_mask,
    _mm512_min_epu32,
    _mm512_max_epu32
);
compares!(
    i64,
    _mm512_cmplt_epi64_mask,
    _mm512_cmple_epi64_mask,
    _mm512_min_epi64,
    _mm512_max_epi64
);
compares!(
    u64,
    _mm512_cmplt_epu64_mask,
    _mm512_cmple_epu64_mask,
    _mm512_min_epu64,
    _mm512_max_epu64
);

macro_rules! native_saturating {
    ($element:ty, $add:ident, $sub:ident) => {
        impl IntOps<__m512i> for $element {
            #[inline(always)]
            fn saturating_add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
                unsafe { $add(a, b) }
            }

            #[inline(always)]
            fn saturating_sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
                unsafe { $sub(a, b) }
            }
        }
    };
}

native_saturating!(u8, _mm512_adds_epu8, _mm512_subs_epu8);
native_saturating!(u16, _mm512_adds_epu16, _mm512_subs_epu16);

impl IntOps<__m512i> for i8 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_adds_epi8(a, b) }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_subs_epi8(a, b) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        let shift = shift.min(7);
        let logical = <u8 as LaneOps<__m512i>>::shr::<S>(a, shift);
        unsafe {
            let sign = _mm512_set1_epi8((0x80u8 >> shift) as i8);
            _mm512_sub_epi8(_mm512_xor_si512(logical, sign), sign)
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        byte_shift_right_arithmetic(a, counts)
    }
}

impl IntOps<__m512i> for i16 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_adds_epi16(a, b) }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_subs_epi16(a, b) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        unsafe { _mm512_sra_epi16(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe { _mm512_srav_epi16(a, counts) }
    }
}

impl IntOps<__m512i> for i32 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe {
            let sum = _mm512_add_epi32(a, b);
            let overflow = _mm512_movepi32_mask(_mm512_and_si512(
                _mm512_xor_si512(a, sum),
                _mm512_xor_si512(b, sum),
            ));
            let saturated =
                _mm512_xor_si512(_mm512_srai_epi32::<31>(a), _mm512_set1_epi32(i32::MAX));
            _mm512_mask_blend_epi32(overflow, sum, saturated)
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe {
            let difference = _mm512_sub_epi32(a, b);
            let overflow = _mm512_movepi32_mask(_mm512_and_si512(
                _mm512_xor_si512(a, b),
                _mm512_xor_si512(a, difference),
            ));
            let saturated =
                _mm512_xor_si512(_mm512_srai_epi32::<31>(a), _mm512_set1_epi32(i32::MAX));
            _mm512_mask_blend_epi32(overflow, difference, saturated)
        }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        unsafe { _mm512_sra_epi32(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe { _mm512_srav_epi32(a, counts) }
    }
}

impl IntOps<__m512i> for u32 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe {
            let sum = _mm512_add_epi32(a, b);
            _mm512_mask_blend_epi32(_mm512_cmplt_epu32_mask(sum, a), sum, _mm512_set1_epi32(-1))
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_maskz_sub_epi32(_mm512_cmple_epu32_mask(b, a), a, b) }
    }
}

impl IntOps<__m512i> for i64 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe {
            let sum = _mm512_add_epi64(a, b);
            let overflow = _mm512_movepi64_mask(_mm512_and_si512(
                _mm512_xor_si512(a, sum),
                _mm512_xor_si512(b, sum),
            ));
            let saturated =
                _mm512_xor_si512(_mm512_srai_epi64::<63>(a), _mm512_set1_epi64(i64::MAX));
            _mm512_mask_blend_epi64(overflow, sum, saturated)
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe {
            let difference = _mm512_sub_epi64(a, b);
            let overflow = _mm512_movepi64_mask(_mm512_and_si512(
                _mm512_xor_si512(a, b),
                _mm512_xor_si512(a, difference),
            ));
            let saturated =
                _mm512_xor_si512(_mm512_srai_epi64::<63>(a), _mm512_set1_epi64(i64::MAX));
            _mm512_mask_blend_epi64(overflow, difference, saturated)
        }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m512i, shift: u32) -> __m512i {
        unsafe { _mm512_sra_epi64(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m512i, counts: __m512i) -> __m512i {
        unsafe { _mm512_srav_epi64(a, counts) }
    }
}

impl IntOps<__m512i> for u64 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe {
            let sum = _mm512_add_epi64(a, b);
            _mm512_mask_blend_epi64(_mm512_cmplt_epu64_mask(sum, a), sum, _mm512_set1_epi64(-1))
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m512i, b: __m512i) -> __m512i {
        unsafe { _mm512_maskz_sub_epi64(_mm512_cmple_epu64_mask(b, a), a, b) }
    }
}

impl NegOps<__m512i> for i8 {}
impl NegOps<__m512i> for i16 {}
impl NegOps<__m512i> for i32 {}
impl NegOps<__m512i> for i64 {}

macro_rules! absolute {
    ($element:ty, $abs:ident) => {
        impl SignedOps<__m512i> for $element {
            #[inline(always)]
            fn abs<S: Simd>(a: __m512i) -> __m512i {
                unsafe { $abs(a) }
            }
        }
    };
}

absolute!(i8, _mm512_abs_epi8);
absolute!(i16, _mm512_abs_epi16);
absolute!(i32, _mm512_abs_epi32);
absolute!(i64, _mm512_abs_epi64);

macro_rules! widen {
    ($element:ty, $wide:ty, $convert:ident) => {
        impl WidenOps<__m512i> for $element {
            type Wide = $wide;

            #[inline(always)]
            fn widen_low<S: Simd>(a: __m512i) -> __m512i {
                unsafe { $convert(low(a)) }
            }

            #[inline(always)]
            fn widen_high<S: Simd>(a: __m512i) -> __m512i {
                unsafe { $convert(high(a)) }
            }
        }
    };
}

widen!(i8, i16, _mm512_cvtepi8_epi16);
widen!(u8, u16, _mm512_cvtepu8_epi16);
widen!(i16, i32, _mm512_cvtepi16_epi32);
widen!(u16, u32, _mm512_cvtepu16_epi32);
widen!(i32, i64, _mm512_cvtepi32_epi64);
widen!(u32, u64, _mm512_cvtepu32_epi64);

macro_rules! narrow {
    ($trait:ident, $method:ident, $associated:ident, $element:ty, $narrow:ty, |$value:ident| $prepare:expr, $convert:ident) => {
        impl $trait<__m512i> for $element {
            type $associated = $narrow;

            #[inline(always)]
            fn $method<S: Simd>(low: __m512i, high: __m512i) -> __m512i {
                unsafe {
                    let convert = |$value: __m512i| $convert($prepare);
                    join(convert(low), convert(high))
                }
            }
        }
    };
}

narrow!(
    NarrowOps,
    narrow,
    Narrow,
    i16,
    i8,
    |x| x,
    _mm512_cvtsepi16_epi8
);
narrow!(
    NarrowOps,
    narrow,
    Narrow,
    u16,
    u8,
    |x| x,
    _mm512_cvtusepi16_epi8
);
narrow!(
    NarrowOps,
    narrow,
    Narrow,
    i32,
    i16,
    |x| x,
    _mm512_cvtsepi32_epi16
);
narrow!(
    NarrowOps,
    narrow,
    Narrow,
    u32,
    u16,
    |x| x,
    _mm512_cvtusepi32_epi16
);
narrow!(
    NarrowOps,
    narrow,
    Narrow,
    i64,
    i32,
    |x| x,
    _mm512_cvtsepi64_epi32
);
narrow!(
    NarrowOps,
    narrow,
    Narrow,
    u64,
    u32,
    |x| x,
    _mm512_cvtusepi64_epi32
);
narrow!(
    NarrowUnsignedOps,
    narrow_unsigned,
    NarrowUnsigned,
    i16,
    u8,
    |x| _mm512_max_epi16(x, _mm512_setzero_si512()),
    _mm512_cvtusepi16_epi8
);
narrow!(
    NarrowUnsignedOps,
    narrow_unsigned,
    NarrowUnsigned,
    i32,
    u16,
    |x| _mm512_max_epi32(x, _mm512_setzero_si512()),
    _mm512_cvtusepi32_epi16
);
narrow!(
    NarrowUnsignedOps,
    narrow_unsigned,
    NarrowUnsigned,
    i64,
    u32,
    |x| _mm512_max_epi64(x, _mm512_setzero_si512()),
    _mm512_cvtusepi64_epi32
);

impl SwizzleOps<__m512i> for u8 {
    #[inline(always)]
    fn swizzle<S: Simd>(table: __m512i, indices: __m512i) -> __m512i {
        unsafe {
            let local = _mm512_and_si512(indices, _mm512_set1_epi8(0x0F));
            let block = |chosen: __m512i| _mm512_shuffle_epi8(chosen, local);
            let first = block(_mm512_shuffle_i32x4::<0x00>(table, table));
            let second = block(_mm512_shuffle_i32x4::<0x55>(table, table));
            let third = block(_mm512_shuffle_i32x4::<0xAA>(table, table));
            let fourth = block(_mm512_shuffle_i32x4::<0xFF>(table, table));
            let odd_block = _mm512_test_epi8_mask(indices, _mm512_set1_epi8(0x10));
            let upper_half = _mm512_test_epi8_mask(indices, _mm512_set1_epi8(0x20));
            let lower = _mm512_mask_blend_epi8(odd_block, first, second);
            let upper = _mm512_mask_blend_epi8(odd_block, third, fourth);
            let picked = _mm512_mask_blend_epi8(upper_half, lower, upper);
            _mm512_maskz_mov_epi8(
                _mm512_cmplt_epu8_mask(indices, _mm512_set1_epi8(64)),
                picked,
            )
        }
    }
}
