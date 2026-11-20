use std::arch::x86_64::*;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div, DivAssign,
    Mul, MulAssign, Neg, Not, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

use crate::element::{FloatElement, Lane};
use crate::mask::fmt_mask;
use crate::sealed::Sealed;
use crate::vector::{assert_loadable, fmt_lanes};
use crate::{Compress, Element, Float, Gather, Int, Mask, ScalarFloat, SignedInt, Simd, Vector};

pub struct Single<S, E> {
    value: E,
    marker: PhantomData<fn() -> S>,
}

pub struct SingleMask<S, L> {
    set: bool,
    marker: PhantomData<fn() -> (S, L)>,
}

impl<S, E: Copy> Clone for Single<S, E> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, E: Copy> Copy for Single<S, E> {}

impl<S, L> Clone for SingleMask<S, L> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, L> Copy for SingleMask<S, L> {}

impl<S, E> Single<S, E> {
    #[inline(always)]
    const fn new(value: E) -> Self {
        Self {
            value,
            marker: PhantomData,
        }
    }
}

impl<S, L> SingleMask<S, L> {
    #[inline(always)]
    const fn new(set: bool) -> Self {
        Self {
            set,
            marker: PhantomData,
        }
    }
}

impl<S, E> Sealed for Single<S, E> {}
impl<S, L> Sealed for SingleMask<S, L> {}

pub trait ScalarOps: Element {
    fn add(a: Self, b: Self) -> Self;
    fn sub(a: Self, b: Self) -> Self;
    fn mul(a: Self, b: Self) -> Self;
    fn less(a: Self, b: Self) -> bool;
    fn less_equal(a: Self, b: Self) -> bool;
    fn minimum(a: Self, b: Self) -> Self;
    fn maximum(a: Self, b: Self) -> Self;
}

pub trait ScalarIntOps: ScalarOps {
    fn shift_left(a: Self, count: u32) -> Self;
    fn shift_right(a: Self, count: u32) -> Self;
    fn shift_left_each(a: Self, count: Self) -> Self;
    fn shift_right_each(a: Self, count: Self) -> Self;
    fn and(a: Self, b: Self) -> Self;
    fn or(a: Self, b: Self) -> Self;
    fn xor(a: Self, b: Self) -> Self;
    fn complement(a: Self) -> Self;
    fn saturating_add(a: Self, b: Self) -> Self;
    fn saturating_sub(a: Self, b: Self) -> Self;
    fn count_ones(a: Self) -> Self;
}

pub trait ScalarNegOps: ScalarOps {
    fn neg(a: Self) -> Self;
}

pub trait ScalarSignedOps: ScalarIntOps + ScalarNegOps {
    fn abs(a: Self) -> Self;
}

pub trait ScalarFloatOps:
    ScalarOps + ScalarNegOps + FloatElement<Bits: ScalarIntOps, Int: ScalarSignedOps>
{
    fn div(a: Self, b: Self) -> Self;
    fn sqrt(a: Self) -> Self;
    fn recip_estimate<S: Simd>(a: Self) -> Self;
    fn rsqrt_estimate<S: Simd>(a: Self) -> Self;
    fn mul_add<S: Simd>(a: Self, b: Self, c: Self) -> Self;
    fn abs(a: Self) -> Self;
    fn copysign(a: Self, sign: Self) -> Self;
    fn floor(a: Self) -> Self;
    fn ceil(a: Self) -> Self;
    fn round(a: Self) -> Self;
    fn trunc(a: Self) -> Self;
    fn is_nan(a: Self) -> bool;
    fn to_int_trunc(a: Self) -> <Self as FloatElement>::Int;
    fn to_int_round(a: Self) -> <Self as FloatElement>::Int;
    fn from_int(a: <Self as FloatElement>::Int) -> Self;
}

pub trait ScalarGatherOps: ScalarOps {
    type Index: ScalarIntOps + Element<Lane = Self::Lane>;

    fn index(index: Self::Index) -> usize;
}

#[inline(always)]
fn shift_count(count: u64) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

macro_rules! scalar_ints {
    ($($element:ty => $bits:expr),*) => {
        $(
            impl ScalarOps for $element {
                #[inline(always)]
                fn add(a: Self, b: Self) -> Self {
                    a.wrapping_add(b)
                }

                #[inline(always)]
                fn sub(a: Self, b: Self) -> Self {
                    a.wrapping_sub(b)
                }

                #[inline(always)]
                fn mul(a: Self, b: Self) -> Self {
                    a.wrapping_mul(b)
                }

                #[inline(always)]
                fn less(a: Self, b: Self) -> bool {
                    a < b
                }

                #[inline(always)]
                fn less_equal(a: Self, b: Self) -> bool {
                    a <= b
                }

                #[inline(always)]
                fn minimum(a: Self, b: Self) -> Self {
                    Ord::min(a, b)
                }

                #[inline(always)]
                fn maximum(a: Self, b: Self) -> Self {
                    Ord::max(a, b)
                }
            }

            impl ScalarIntOps for $element {
                #[inline(always)]
                fn shift_left(a: Self, count: u32) -> Self {
                    a.checked_shl(count).unwrap_or(0)
                }

                #[inline(always)]
                fn shift_right(a: Self, count: u32) -> Self {
                    if <$element>::MIN == 0 {
                        a.checked_shr(count).unwrap_or(0)
                    } else {
                        a >> count.min($bits - 1)
                    }
                }

                #[inline(always)]
                fn shift_left_each(a: Self, count: Self) -> Self {
                    Self::shift_left(a, shift_count(count.to_lane().to_u64()))
                }

                #[inline(always)]
                fn shift_right_each(a: Self, count: Self) -> Self {
                    Self::shift_right(a, shift_count(count.to_lane().to_u64()))
                }

                #[inline(always)]
                fn and(a: Self, b: Self) -> Self {
                    a & b
                }

                #[inline(always)]
                fn or(a: Self, b: Self) -> Self {
                    a | b
                }

                #[inline(always)]
                fn xor(a: Self, b: Self) -> Self {
                    a ^ b
                }

                #[inline(always)]
                fn complement(a: Self) -> Self {
                    !a
                }

                #[inline(always)]
                fn saturating_add(a: Self, b: Self) -> Self {
                    a.saturating_add(b)
                }

                #[inline(always)]
                fn saturating_sub(a: Self, b: Self) -> Self {
                    a.saturating_sub(b)
                }

                #[inline(always)]
                fn count_ones(a: Self) -> Self {
                    a.count_ones() as $element
                }
            }
        )*
    };
}

scalar_ints!(i8 => 8, u8 => 8, i16 => 16, u16 => 16, i32 => 32, u32 => 32, i64 => 64, u64 => 64);

macro_rules! scalar_signed {
    ($($element:ty),*) => {
        $(
            impl ScalarNegOps for $element {
                #[inline(always)]
                fn neg(a: Self) -> Self {
                    a.wrapping_neg()
                }
            }

            impl ScalarSignedOps for $element {
                #[inline(always)]
                fn abs(a: Self) -> Self {
                    a.wrapping_abs()
                }
            }
        )*
    };
}

scalar_signed!(i8, i16, i32, i64);

macro_rules! scalar_floats {
    (
        $element:ty, bits: $bits:ty, int: $int:ty,
        set: $set:ident, from_packed: $from_packed:ident,
        rcp: $rcp:expr, rcp14: $rcp14:expr, rsqrt: $rsqrt:expr, rsqrt14: $rsqrt14:expr
    ) => {
        impl ScalarOps for $element {
            #[inline(always)]
            fn add(a: Self, b: Self) -> Self {
                a + b
            }

            #[inline(always)]
            fn sub(a: Self, b: Self) -> Self {
                a - b
            }

            #[inline(always)]
            fn mul(a: Self, b: Self) -> Self {
                a * b
            }

            #[inline(always)]
            fn less(a: Self, b: Self) -> bool {
                a < b
            }

            #[inline(always)]
            fn less_equal(a: Self, b: Self) -> bool {
                a <= b
            }

            #[inline(always)]
            fn minimum(a: Self, b: Self) -> Self {
                if a < b { a } else { b }
            }

            #[inline(always)]
            fn maximum(a: Self, b: Self) -> Self {
                if a > b { a } else { b }
            }
        }

        impl ScalarNegOps for $element {
            #[inline(always)]
            fn neg(a: Self) -> Self {
                -a
            }
        }

        impl ScalarFloatOps for $element {
            #[inline(always)]
            fn div(a: Self, b: Self) -> Self {
                a / b
            }

            #[inline(always)]
            fn sqrt(a: Self) -> Self {
                a.sqrt()
            }

            #[inline(always)]
            fn recip_estimate<S: Simd>(a: Self) -> Self {
                unsafe {
                    let packed = $set(a);
                    if S::AVX512 {
                        $from_packed($rcp14(packed))
                    } else {
                        $from_packed($rcp(packed))
                    }
                }
            }

            #[inline(always)]
            fn rsqrt_estimate<S: Simd>(a: Self) -> Self {
                unsafe {
                    let packed = $set(a);
                    if S::AVX512 {
                        $from_packed($rsqrt14(packed))
                    } else {
                        $from_packed($rsqrt(packed))
                    }
                }
            }

            #[inline(always)]
            fn mul_add<S: Simd>(a: Self, b: Self, c: Self) -> Self {
                if S::AVX2 { a.mul_add(b, c) } else { a * b + c }
            }

            #[inline(always)]
            fn abs(a: Self) -> Self {
                a.abs()
            }

            #[inline(always)]
            fn copysign(a: Self, sign: Self) -> Self {
                a.copysign(sign)
            }

            #[inline(always)]
            fn floor(a: Self) -> Self {
                a.floor()
            }

            #[inline(always)]
            fn ceil(a: Self) -> Self {
                a.ceil()
            }

            #[inline(always)]
            fn round(a: Self) -> Self {
                a.round_ties_even()
            }

            #[inline(always)]
            fn trunc(a: Self) -> Self {
                a.trunc()
            }

            #[inline(always)]
            fn is_nan(a: Self) -> bool {
                a.is_nan()
            }

            #[inline(always)]
            fn to_int_trunc(a: Self) -> $int {
                ScalarFloat::to_int_trunc(a)
            }

            #[inline(always)]
            fn to_int_round(a: Self) -> $int {
                ScalarFloat::to_int_round(a)
            }

            #[inline(always)]
            fn from_int(a: $int) -> Self {
                a as $element
            }
        }

        impl ScalarGatherOps for $element {
            type Index = $bits;

            #[inline(always)]
            fn index(index: $bits) -> usize {
                index as usize
            }
        }

        impl ScalarGatherOps for $bits {
            type Index = $bits;

            #[inline(always)]
            fn index(index: $bits) -> usize {
                index as usize
            }
        }

        impl ScalarGatherOps for $int {
            type Index = $bits;

            #[inline(always)]
            fn index(index: $bits) -> usize {
                index as usize
            }
        }
    };
}

#[inline(always)]
fn first_ps(packed: __m128) -> f32 {
    unsafe { _mm_cvtss_f32(packed) }
}

#[inline(always)]
fn first_pd(packed: __m128d) -> f64 {
    unsafe { _mm_cvtsd_f64(packed) }
}

#[inline(always)]
fn sqrt_recip_pd(packed: __m128d) -> __m128d {
    unsafe { _mm_div_pd(_mm_set1_pd(1.0), _mm_sqrt_pd(packed)) }
}

#[inline(always)]
fn recip_pd(packed: __m128d) -> __m128d {
    unsafe { _mm_div_pd(_mm_set1_pd(1.0), packed) }
}

scalar_floats!(
    f32, bits: u32, int: i32,
    set: _mm_set_ss, from_packed: first_ps,
    rcp: _mm_rcp_ps, rcp14: _mm_rcp14_ps, rsqrt: _mm_rsqrt_ps, rsqrt14: _mm_rsqrt14_ps
);

scalar_floats!(
    f64, bits: u64, int: i64,
    set: _mm_set_sd, from_packed: first_pd,
    rcp: recip_pd, rcp14: _mm_rcp14_pd, rsqrt: sqrt_recip_pd, rsqrt14: _mm_rsqrt14_pd
);

impl<S: Simd, E: ScalarOps> Vector for Single<S, E> {
    type Simd = S;
    type Elem = E;
    type Mask = SingleMask<S, E::Lane>;
    type Raw = E::Lane;
    const LANES: usize = 1;

    #[inline(always)]
    fn splat(value: E) -> Self {
        Self::new(value)
    }

    #[inline(always)]
    fn load(slice: &[E]) -> Self {
        assert_loadable(slice.len(), 1);
        Self::new(slice[0])
    }

    #[inline(always)]
    fn store(self, slice: &mut [E]) {
        assert_loadable(slice.len(), 1);
        slice[0] = self.value;
    }

    #[inline(always)]
    fn load_partial(slice: &[E]) -> Self {
        Self::new(slice.first().copied().unwrap_or_default())
    }

    #[inline(always)]
    fn store_partial(self, slice: &mut [E]) {
        if let Some(target) = slice.first_mut() {
            *target = self.value;
        }
    }

    #[inline(always)]
    fn select(mask: Self::Mask, if_true: Self, if_false: Self) -> Self {
        if mask.set { if_true } else { if_false }
    }

    #[inline(always)]
    fn simd_eq(self, other: Self) -> Self::Mask {
        SingleMask::new(self.value == other.value)
    }

    #[inline(always)]
    fn reverse(self) -> Self {
        self
    }

    #[inline(always)]
    fn interleave(self, other: Self) -> (Self, Self) {
        (self, other)
    }

    #[inline(always)]
    fn deinterleave(self, other: Self) -> (Self, Self) {
        (self, other)
    }

    #[inline(always)]
    fn to_raw(self) -> E::Lane {
        self.value.to_lane()
    }

    #[inline(always)]
    fn from_raw(raw: E::Lane) -> Self {
        Self::new(E::from_lane(raw))
    }
}

impl<S: Simd, E: ScalarOps> Default for Single<S, E> {
    #[inline(always)]
    fn default() -> Self {
        Self::new(E::default())
    }
}

impl<S: Simd, E: ScalarOps> PartialEq for Single<S, E> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<S: Simd, E: ScalarOps> fmt::Debug for Single<S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_lanes(*self, f)
    }
}

impl<S: Simd, L: 'static> Mask for SingleMask<S, L> {
    type Simd = S;
    const LANES: usize = 1;

    #[inline(always)]
    fn splat(value: bool) -> Self {
        Self::new(value)
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> Self {
        Self::new(bits & 1 != 0)
    }

    #[inline(always)]
    fn to_bitmask(self) -> u64 {
        u64::from(self.set)
    }

    #[inline(always)]
    fn any(self) -> bool {
        self.set
    }

    #[inline(always)]
    fn all(self) -> bool {
        self.set
    }
}

impl<S: Simd, L: 'static> Default for SingleMask<S, L> {
    #[inline(always)]
    fn default() -> Self {
        Self::new(false)
    }
}

impl<S: Simd, L: 'static> PartialEq for SingleMask<S, L> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.set == other.set
    }
}

impl<S: Simd, L: 'static> Eq for SingleMask<S, L> {}

impl<S: Simd, L: 'static> fmt::Debug for SingleMask<S, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_mask(*self, f)
    }
}

impl<S: Simd, L: 'static> Not for SingleMask<S, L> {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self {
        Self::new(!self.set)
    }
}

macro_rules! mask_operator {
    ($trait:ident, $method:ident, $assign:ident, $assign_method:ident, $op:tt) => {
        impl<S: Simd, L: 'static> $trait for SingleMask<S, L> {
            type Output = Self;

            #[inline(always)]
            fn $method(self, other: Self) -> Self {
                Self::new(self.set $op other.set)
            }
        }

        impl<S: Simd, L: 'static> $assign for SingleMask<S, L> {
            #[inline(always)]
            fn $assign_method(&mut self, other: Self) {
                *self = $trait::$method(*self, other);
            }
        }
    };
}

mask_operator!(BitAnd, bitand, BitAndAssign, bitand_assign, &);
mask_operator!(BitOr, bitor, BitOrAssign, bitor_assign, |);
mask_operator!(BitXor, bitxor, BitXorAssign, bitxor_assign, ^);

macro_rules! operator {
    ($bound:ident, $trait:ident, $method:ident, $assign:ident, $assign_method:ident, $op:expr) => {
        impl<S: Simd, E: $bound> $trait for Single<S, E> {
            type Output = Self;

            #[inline(always)]
            fn $method(self, other: Self) -> Self {
                Self::new($op(self.value, other.value))
            }
        }

        impl<S: Simd, E: $bound> $assign for Single<S, E> {
            #[inline(always)]
            fn $assign_method(&mut self, other: Self) {
                *self = $trait::$method(*self, other);
            }
        }
    };
}

operator!(ScalarOps, Add, add, AddAssign, add_assign, E::add);
operator!(ScalarOps, Sub, sub, SubAssign, sub_assign, E::sub);
operator!(ScalarOps, Mul, mul, MulAssign, mul_assign, E::mul);
operator!(ScalarFloatOps, Div, div, DivAssign, div_assign, E::div);
operator!(
    ScalarIntOps,
    BitAnd,
    bitand,
    BitAndAssign,
    bitand_assign,
    E::and
);
operator!(ScalarIntOps, BitOr, bitor, BitOrAssign, bitor_assign, E::or);
operator!(
    ScalarIntOps,
    BitXor,
    bitxor,
    BitXorAssign,
    bitxor_assign,
    E::xor
);
operator!(
    ScalarIntOps,
    Shl,
    shl,
    ShlAssign,
    shl_assign,
    E::shift_left_each
);
operator!(
    ScalarIntOps,
    Shr,
    shr,
    ShrAssign,
    shr_assign,
    E::shift_right_each
);

impl<S: Simd, E: ScalarIntOps> Not for Single<S, E> {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self {
        Self::new(E::complement(self.value))
    }
}

impl<S: Simd, E: ScalarIntOps> Shl<u32> for Single<S, E> {
    type Output = Self;

    #[inline(always)]
    fn shl(self, count: u32) -> Self {
        Self::new(E::shift_left(self.value, count))
    }
}

impl<S: Simd, E: ScalarIntOps> Shr<u32> for Single<S, E> {
    type Output = Self;

    #[inline(always)]
    fn shr(self, count: u32) -> Self {
        Self::new(E::shift_right(self.value, count))
    }
}

impl<S: Simd, E: ScalarIntOps> ShlAssign<u32> for Single<S, E> {
    #[inline(always)]
    fn shl_assign(&mut self, count: u32) {
        *self = *self << count;
    }
}

impl<S: Simd, E: ScalarIntOps> ShrAssign<u32> for Single<S, E> {
    #[inline(always)]
    fn shr_assign(&mut self, count: u32) {
        *self = *self >> count;
    }
}

impl<S: Simd, E: ScalarNegOps> Neg for Single<S, E> {
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self {
        Self::new(E::neg(self.value))
    }
}

impl<S: Simd, E: ScalarFloatOps> Float for Single<S, E> {
    type Bits = Single<S, <E as FloatElement>::Bits>;
    type Int = Single<S, <E as FloatElement>::Int>;

    #[inline(always)]
    fn mul_add(self, factor: Self, addend: Self) -> Self {
        Self::new(E::mul_add::<S>(self.value, factor.value, addend.value))
    }

    #[inline(always)]
    fn mul_sub(self, factor: Self, subtrahend: Self) -> Self {
        Self::new(E::mul_add::<S>(
            self.value,
            factor.value,
            E::neg(subtrahend.value),
        ))
    }

    #[inline(always)]
    fn neg_mul_add(self, factor: Self, addend: Self) -> Self {
        Self::new(E::mul_add::<S>(
            E::neg(self.value),
            factor.value,
            addend.value,
        ))
    }

    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::new(E::minimum(self.value, other.value))
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::new(E::maximum(self.value, other.value))
    }

    #[inline(always)]
    fn abs(self) -> Self {
        Self::new(E::abs(self.value))
    }

    #[inline(always)]
    fn copysign(self, sign: Self) -> Self {
        Self::new(E::copysign(self.value, sign.value))
    }

    #[inline(always)]
    fn sqrt(self) -> Self {
        Self::new(E::sqrt(self.value))
    }

    #[inline(always)]
    fn recip_estimate(self) -> Self {
        Self::new(E::recip_estimate::<S>(self.value))
    }

    #[inline(always)]
    fn rsqrt_estimate(self) -> Self {
        Self::new(E::rsqrt_estimate::<S>(self.value))
    }

    #[inline(always)]
    fn floor(self) -> Self {
        Self::new(E::floor(self.value))
    }

    #[inline(always)]
    fn ceil(self) -> Self {
        Self::new(E::ceil(self.value))
    }

    #[inline(always)]
    fn round(self) -> Self {
        Self::new(E::round(self.value))
    }

    #[inline(always)]
    fn trunc(self) -> Self {
        Self::new(E::trunc(self.value))
    }

    #[inline(always)]
    fn simd_lt(self, other: Self) -> Self::Mask {
        SingleMask::new(E::less(self.value, other.value))
    }

    #[inline(always)]
    fn simd_le(self, other: Self) -> Self::Mask {
        SingleMask::new(E::less_equal(self.value, other.value))
    }

    #[inline(always)]
    fn is_nan(self) -> Self::Mask {
        SingleMask::new(E::is_nan(self.value))
    }

    #[inline(always)]
    fn reduce_add(self) -> E {
        self.value
    }

    #[inline(always)]
    fn reduce_min(self) -> E {
        self.value
    }

    #[inline(always)]
    fn reduce_max(self) -> E {
        self.value
    }

    #[inline(always)]
    fn prefix_sum(self) -> Self {
        self
    }

    #[inline(always)]
    fn to_int_trunc(self) -> Self::Int {
        Single::new(E::to_int_trunc(self.value))
    }

    #[inline(always)]
    fn to_int_round(self) -> Self::Int {
        Single::new(E::to_int_round(self.value))
    }

    #[inline(always)]
    fn from_int(value: Self::Int) -> Self {
        Self::new(E::from_int(value.value))
    }

    #[inline(always)]
    fn load_transposed(rows: &[&[E; 4]]) -> [Self; 4] {
        assert_loadable(rows.len(), 1);
        rows[0].map(Self::new)
    }

    #[inline(always)]
    fn store_transposed(columns: [Self; 4], rows: &mut [&mut [E; 4]]) {
        assert_loadable(rows.len(), 1);
        *rows[0] = columns.map(|column| column.value);
    }
}

impl<S: Simd, E: ScalarIntOps> Int for Single<S, E> {
    #[inline(always)]
    fn simd_lt(self, other: Self) -> Self::Mask {
        SingleMask::new(E::less(self.value, other.value))
    }

    #[inline(always)]
    fn simd_le(self, other: Self) -> Self::Mask {
        SingleMask::new(E::less_equal(self.value, other.value))
    }

    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::new(E::minimum(self.value, other.value))
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::new(E::maximum(self.value, other.value))
    }

    #[inline(always)]
    fn saturating_add(self, other: Self) -> Self {
        Self::new(E::saturating_add(self.value, other.value))
    }

    #[inline(always)]
    fn saturating_sub(self, other: Self) -> Self {
        Self::new(E::saturating_sub(self.value, other.value))
    }

    #[inline(always)]
    fn count_ones(self) -> Self {
        Self::new(E::count_ones(self.value))
    }

    #[inline(always)]
    fn reduce_add(self) -> E {
        self.value
    }

    #[inline(always)]
    fn reduce_min(self) -> E {
        self.value
    }

    #[inline(always)]
    fn reduce_max(self) -> E {
        self.value
    }

    #[inline(always)]
    fn prefix_sum(self) -> Self {
        self
    }
}

impl<S: Simd, E: ScalarSignedOps> SignedInt for Single<S, E> {
    #[inline(always)]
    fn abs(self) -> Self {
        Self::new(E::abs(self.value))
    }
}

impl<S: Simd, E: ScalarGatherOps> Gather for Single<S, E> {
    type Index = Single<S, E::Index>;

    #[inline(always)]
    fn gather(slice: &[E], indices: Self::Index) -> Self {
        Self::new(slice[E::index(indices.value)])
    }
}

impl<S: Simd, E: ScalarGatherOps> Compress for Single<S, E> {
    #[inline(always)]
    fn compress(self, mask: Self::Mask) -> Self {
        if mask.set { self } else { Self::default() }
    }
}
