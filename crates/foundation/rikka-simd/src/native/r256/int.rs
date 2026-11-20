use std::arch::x86_64::*;

use super::lanes::byte_shift_right_arithmetic;
use super::{LANE_ORDER, blend, from_ps, high, join, low, not, ps, shift_count};
use crate::Simd;
use crate::native::map_lanes;
use crate::native::ops::{
    ElemOps, IntOps, LaneOps, NarrowOps, NarrowUnsignedOps, NegOps, SignedOps, SwizzleOps, WidenOps,
};

#[inline(always)]
fn flip(a: __m256i, sign: __m256i) -> __m256i {
    unsafe { _mm256_xor_si256(a, sign) }
}

#[inline(always)]
fn sign8() -> __m256i {
    unsafe { _mm256_set1_epi8(i8::MIN) }
}

#[inline(always)]
fn sign16() -> __m256i {
    unsafe { _mm256_set1_epi16(i16::MIN) }
}

#[inline(always)]
fn sign32() -> __m256i {
    unsafe { _mm256_set1_epi32(i32::MIN) }
}

#[inline(always)]
fn sign64() -> __m256i {
    unsafe { _mm256_set1_epi64x(i64::MIN) }
}

#[inline(always)]
fn negative32(a: __m256i) -> __m256i {
    unsafe { _mm256_srai_epi32::<31>(a) }
}

#[inline(always)]
fn negative64(a: __m256i) -> __m256i {
    unsafe { _mm256_cmpgt_epi64(_mm256_setzero_si256(), a) }
}

#[inline(always)]
fn lt_u64<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
    unsafe {
        if S::AVX512 {
            _mm256_movm_epi64(_mm256_cmplt_epu64_mask(a, b))
        } else {
            _mm256_cmpgt_epi64(flip(b, sign64()), flip(a, sign64()))
        }
    }
}

#[inline(always)]
fn lanes_in_order(packed: __m256i) -> __m256i {
    unsafe { _mm256_permute4x64_epi64::<LANE_ORDER>(packed) }
}

#[inline(always)]
fn low_halves(low_lanes: __m256i, high_lanes: __m256i) -> __m256i {
    unsafe {
        lanes_in_order(from_ps(_mm256_shuffle_ps::<0x88>(
            ps(low_lanes),
            ps(high_lanes),
        )))
    }
}

macro_rules! reductions {
    ($element:ty) => {
        #[inline(always)]
        fn reduce_min<S: Simd>(a: __m256i) -> $element {
            let halves = <$element as ElemOps<__m128i>>::minimum::<S>(low(a), high(a));
            <$element as ElemOps<__m128i>>::reduce_min::<S>(halves)
        }

        #[inline(always)]
        fn reduce_max<S: Simd>(a: __m256i) -> $element {
            let halves = <$element as ElemOps<__m128i>>::maximum::<S>(low(a), high(a));
            <$element as ElemOps<__m128i>>::reduce_max::<S>(halves)
        }
    };
}

macro_rules! native_compares {
    ($element:ty, $greater:ident, $min:ident, $max:ident, $flip:expr) => {
        impl ElemOps<__m256i> for $element {
            reductions!($element);

            #[inline(always)]
            fn less<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
                unsafe { $greater(flip(b, $flip), flip(a, $flip)) }
            }

            #[inline(always)]
            fn less_equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
                unsafe { not($greater(flip(a, $flip), flip(b, $flip))) }
            }

            #[inline(always)]
            fn minimum<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
                unsafe { $min(a, b) }
            }

            #[inline(always)]
            fn maximum<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
                unsafe { $max(a, b) }
            }
        }
    };
}

native_compares!(
    i8,
    _mm256_cmpgt_epi8,
    _mm256_min_epi8,
    _mm256_max_epi8,
    _mm256_setzero_si256()
);
native_compares!(
    u8,
    _mm256_cmpgt_epi8,
    _mm256_min_epu8,
    _mm256_max_epu8,
    sign8()
);
native_compares!(
    i16,
    _mm256_cmpgt_epi16,
    _mm256_min_epi16,
    _mm256_max_epi16,
    _mm256_setzero_si256()
);
native_compares!(
    u16,
    _mm256_cmpgt_epi16,
    _mm256_min_epu16,
    _mm256_max_epu16,
    sign16()
);
native_compares!(
    i32,
    _mm256_cmpgt_epi32,
    _mm256_min_epi32,
    _mm256_max_epi32,
    _mm256_setzero_si256()
);
native_compares!(
    u32,
    _mm256_cmpgt_epi32,
    _mm256_min_epu32,
    _mm256_max_epu32,
    sign32()
);

impl ElemOps<__m256i> for i64 {
    reductions!(i64);

    #[inline(always)]
    fn less<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_cmpgt_epi64(b, a) }
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { not(_mm256_cmpgt_epi64(a, b)) }
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            if S::AVX512 {
                _mm256_min_epi64(a, b)
            } else {
                blend(_mm256_cmpgt_epi64(a, b), b, a)
            }
        }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            if S::AVX512 {
                _mm256_max_epi64(a, b)
            } else {
                blend(_mm256_cmpgt_epi64(a, b), a, b)
            }
        }
    }
}

impl ElemOps<__m256i> for u64 {
    reductions!(u64);

    #[inline(always)]
    fn less<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        lt_u64::<S>(a, b)
    }

    #[inline(always)]
    fn less_equal<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        not(lt_u64::<S>(b, a))
    }

    #[inline(always)]
    fn minimum<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            if S::AVX512 {
                _mm256_min_epu64(a, b)
            } else {
                blend(lt_u64::<S>(a, b), a, b)
            }
        }
    }

    #[inline(always)]
    fn maximum<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            if S::AVX512 {
                _mm256_max_epu64(a, b)
            } else {
                blend(lt_u64::<S>(a, b), b, a)
            }
        }
    }
}

macro_rules! native_saturating {
    ($element:ty, $add:ident, $sub:ident) => {
        impl IntOps<__m256i> for $element {
            #[inline(always)]
            fn saturating_add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
                unsafe { $add(a, b) }
            }

            #[inline(always)]
            fn saturating_sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
                unsafe { $sub(a, b) }
            }
        }
    };
}

native_saturating!(u8, _mm256_adds_epu8, _mm256_subs_epu8);
native_saturating!(u16, _mm256_adds_epu16, _mm256_subs_epu16);

impl IntOps<__m256i> for i8 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_adds_epi8(a, b) }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_subs_epi8(a, b) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        let shift = shift.min(7);
        let logical = <u8 as LaneOps<__m256i>>::shr::<S>(a, shift);
        unsafe {
            let sign = _mm256_set1_epi8((0x80u8 >> shift) as i8);
            _mm256_sub_epi8(_mm256_xor_si256(logical, sign), sign)
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        byte_shift_right_arithmetic::<S>(a, counts)
    }
}

impl IntOps<__m256i> for i16 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_adds_epi16(a, b) }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_subs_epi16(a, b) }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        unsafe { _mm256_sra_epi16(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe { _mm256_srav_epi16(a, counts) }
        } else {
            map_lanes::<_, i16, 16>(a, counts, |x, n| x >> (n as u16).min(15))
        }
    }
}

impl IntOps<__m256i> for i32 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            let sum = _mm256_add_epi32(a, b);
            let overflow = negative32(_mm256_and_si256(
                _mm256_xor_si256(a, sum),
                _mm256_xor_si256(b, sum),
            ));
            let saturated = _mm256_xor_si256(negative32(a), _mm256_set1_epi32(i32::MAX));
            blend(overflow, saturated, sum)
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            let difference = _mm256_sub_epi32(a, b);
            let overflow = negative32(_mm256_and_si256(
                _mm256_xor_si256(a, b),
                _mm256_xor_si256(a, difference),
            ));
            let saturated = _mm256_xor_si256(negative32(a), _mm256_set1_epi32(i32::MAX));
            blend(overflow, saturated, difference)
        }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        unsafe { _mm256_sra_epi32(a, shift_count(shift)) }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        unsafe { _mm256_srav_epi32(a, counts) }
    }
}

impl IntOps<__m256i> for u32 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            let sum = _mm256_add_epi32(a, b);
            _mm256_or_si256(sum, <Self as ElemOps<__m256i>>::less::<S>(sum, a))
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            _mm256_andnot_si256(
                <Self as ElemOps<__m256i>>::less::<S>(a, b),
                _mm256_sub_epi32(a, b),
            )
        }
    }
}

impl IntOps<__m256i> for i64 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            let sum = _mm256_add_epi64(a, b);
            let overflow = negative64(_mm256_and_si256(
                _mm256_xor_si256(a, sum),
                _mm256_xor_si256(b, sum),
            ));
            let saturated = _mm256_xor_si256(negative64(a), _mm256_set1_epi64x(i64::MAX));
            blend(overflow, saturated, sum)
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            let difference = _mm256_sub_epi64(a, b);
            let overflow = negative64(_mm256_and_si256(
                _mm256_xor_si256(a, b),
                _mm256_xor_si256(a, difference),
            ));
            let saturated = _mm256_xor_si256(negative64(a), _mm256_set1_epi64x(i64::MAX));
            blend(overflow, saturated, difference)
        }
    }

    #[inline(always)]
    fn shr<S: Simd>(a: __m256i, shift: u32) -> __m256i {
        unsafe {
            if S::AVX512 {
                _mm256_sra_epi64(a, shift_count(shift))
            } else {
                let shift = shift.min(63);
                let logical = _mm256_srl_epi64(a, shift_count(shift));
                let sign = _mm256_set1_epi64x(((1u64 << 63) >> shift) as i64);
                _mm256_sub_epi64(_mm256_xor_si256(logical, sign), sign)
            }
        }
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: __m256i, counts: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe { _mm256_srav_epi64(a, counts) }
        } else {
            unsafe {
                let negative = negative64(a);
                let logical = _mm256_srlv_epi64(_mm256_xor_si256(a, negative), counts);
                _mm256_xor_si256(logical, negative)
            }
        }
    }
}

impl IntOps<__m256i> for u64 {
    #[inline(always)]
    fn saturating_add<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe {
            let sum = _mm256_add_epi64(a, b);
            _mm256_or_si256(sum, lt_u64::<S>(sum, a))
        }
    }

    #[inline(always)]
    fn saturating_sub<S: Simd>(a: __m256i, b: __m256i) -> __m256i {
        unsafe { _mm256_andnot_si256(lt_u64::<S>(a, b), _mm256_sub_epi64(a, b)) }
    }
}

impl NegOps<__m256i> for i8 {}
impl NegOps<__m256i> for i16 {}
impl NegOps<__m256i> for i32 {}
impl NegOps<__m256i> for i64 {}

impl SignedOps<__m256i> for i8 {
    #[inline(always)]
    fn abs<S: Simd>(a: __m256i) -> __m256i {
        unsafe { _mm256_abs_epi8(a) }
    }
}

impl SignedOps<__m256i> for i16 {
    #[inline(always)]
    fn abs<S: Simd>(a: __m256i) -> __m256i {
        unsafe { _mm256_abs_epi16(a) }
    }
}

impl SignedOps<__m256i> for i32 {
    #[inline(always)]
    fn abs<S: Simd>(a: __m256i) -> __m256i {
        unsafe { _mm256_abs_epi32(a) }
    }
}

impl SignedOps<__m256i> for i64 {
    #[inline(always)]
    fn abs<S: Simd>(a: __m256i) -> __m256i {
        unsafe {
            if S::AVX512 {
                _mm256_abs_epi64(a)
            } else {
                let negative = negative64(a);
                _mm256_sub_epi64(_mm256_xor_si256(a, negative), negative)
            }
        }
    }
}

macro_rules! widen {
    ($element:ty, $wide:ty, $convert:ident) => {
        impl WidenOps<__m256i> for $element {
            type Wide = $wide;

            #[inline(always)]
            fn widen_low<S: Simd>(a: __m256i) -> __m256i {
                unsafe { $convert(low(a)) }
            }

            #[inline(always)]
            fn widen_high<S: Simd>(a: __m256i) -> __m256i {
                unsafe { $convert(high(a)) }
            }
        }
    };
}

widen!(i8, i16, _mm256_cvtepi8_epi16);
widen!(u8, u16, _mm256_cvtepu8_epi16);
widen!(i16, i32, _mm256_cvtepi16_epi32);
widen!(u16, u32, _mm256_cvtepu16_epi32);
widen!(i32, i64, _mm256_cvtepi32_epi64);
widen!(u32, u64, _mm256_cvtepu32_epi64);

impl NarrowOps<__m256i> for i16 {
    type Narrow = i8;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m256i, high: __m256i) -> __m256i {
        unsafe { lanes_in_order(_mm256_packs_epi16(low, high)) }
    }
}

impl NarrowOps<__m256i> for u16 {
    type Narrow = u8;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m256i, high: __m256i) -> __m256i {
        unsafe {
            let cap = _mm256_set1_epi16(0xFF);
            lanes_in_order(_mm256_packus_epi16(
                _mm256_min_epu16(low, cap),
                _mm256_min_epu16(high, cap),
            ))
        }
    }
}

impl NarrowOps<__m256i> for i32 {
    type Narrow = i16;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m256i, high: __m256i) -> __m256i {
        unsafe { lanes_in_order(_mm256_packs_epi32(low, high)) }
    }
}

impl NarrowOps<__m256i> for u32 {
    type Narrow = u16;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m256i, high: __m256i) -> __m256i {
        unsafe {
            let cap = _mm256_set1_epi32(0xFFFF);
            lanes_in_order(_mm256_packus_epi32(
                _mm256_min_epu32(low, cap),
                _mm256_min_epu32(high, cap),
            ))
        }
    }
}

impl NarrowOps<__m256i> for i64 {
    type Narrow = i32;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m256i, high: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe { join(_mm256_cvtsepi64_epi32(low), _mm256_cvtsepi64_epi32(high)) }
        } else {
            unsafe {
                let lower = _mm256_set1_epi64x(i64::from(i32::MIN));
                let upper = _mm256_set1_epi64x(i64::from(i32::MAX));
                let clamp = |x| {
                    <Self as ElemOps<__m256i>>::minimum::<S>(
                        <Self as ElemOps<__m256i>>::maximum::<S>(x, lower),
                        upper,
                    )
                };
                low_halves(clamp(low), clamp(high))
            }
        }
    }
}

impl NarrowOps<__m256i> for u64 {
    type Narrow = u32;

    #[inline(always)]
    fn narrow<S: Simd>(low: __m256i, high: __m256i) -> __m256i {
        if S::AVX512 {
            unsafe { join(_mm256_cvtusepi64_epi32(low), _mm256_cvtusepi64_epi32(high)) }
        } else {
            let upper = unsafe { _mm256_set1_epi64x(i64::from(u32::MAX)) };
            let clamp = |x| <Self as ElemOps<__m256i>>::minimum::<S>(x, upper);
            low_halves(clamp(low), clamp(high))
        }
    }
}

impl NarrowUnsignedOps<__m256i> for i16 {
    type NarrowUnsigned = u8;

    #[inline(always)]
    fn narrow_unsigned<S: Simd>(low: __m256i, high: __m256i) -> __m256i {
        unsafe { lanes_in_order(_mm256_packus_epi16(low, high)) }
    }
}

impl NarrowUnsignedOps<__m256i> for i32 {
    type NarrowUnsigned = u16;

    #[inline(always)]
    fn narrow_unsigned<S: Simd>(low: __m256i, high: __m256i) -> __m256i {
        unsafe { lanes_in_order(_mm256_packus_epi32(low, high)) }
    }
}

impl NarrowUnsignedOps<__m256i> for i64 {
    type NarrowUnsigned = u32;

    #[inline(always)]
    fn narrow_unsigned<S: Simd>(low: __m256i, high: __m256i) -> __m256i {
        unsafe {
            let lower = _mm256_setzero_si256();
            if S::AVX512 {
                join(
                    _mm256_cvtusepi64_epi32(_mm256_max_epi64(low, lower)),
                    _mm256_cvtusepi64_epi32(_mm256_max_epi64(high, lower)),
                )
            } else {
                let upper = _mm256_set1_epi64x(i64::from(u32::MAX));
                let clamp = |x| {
                    <Self as ElemOps<__m256i>>::minimum::<S>(
                        <Self as ElemOps<__m256i>>::maximum::<S>(x, lower),
                        upper,
                    )
                };
                low_halves(clamp(low), clamp(high))
            }
        }
    }
}

impl SwizzleOps<__m256i> for u8 {
    #[inline(always)]
    fn swizzle<S: Simd>(table: __m256i, indices: __m256i) -> __m256i {
        unsafe {
            let local = _mm256_and_si256(indices, _mm256_set1_epi8(0x0F));
            let from_low =
                _mm256_shuffle_epi8(_mm256_permute2x128_si256::<0x00>(table, table), local);
            let from_high =
                _mm256_shuffle_epi8(_mm256_permute2x128_si256::<0x11>(table, table), local);
            let picked = _mm256_blendv_epi8(from_low, from_high, _mm256_slli_epi16::<3>(indices));
            let in_range =
                _mm256_cmpeq_epi8(_mm256_min_epu8(indices, _mm256_set1_epi8(31)), indices);
            _mm256_and_si256(picked, in_range)
        }
    }
}
