use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

use crate::element::FloatElement;
use crate::math::MathElement;
use crate::{Int, SignedInt, Vector};

pub trait Float:
    Vector<Elem: FloatElement>
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + DivAssign
{
    type Bits: Int<
            Simd = Self::Simd,
            Mask = Self::Mask,
            Raw = Self::Raw,
            Elem = <Self::Elem as FloatElement>::Bits,
        >;
    type Int: SignedInt<
            Simd = Self::Simd,
            Mask = Self::Mask,
            Raw = Self::Raw,
            Elem = <Self::Elem as FloatElement>::Int,
        >;

    fn mul_add(self, factor: Self, addend: Self) -> Self;

    fn mul_sub(self, factor: Self, subtrahend: Self) -> Self;

    fn neg_mul_add(self, factor: Self, addend: Self) -> Self;

    fn min(self, other: Self) -> Self;

    fn max(self, other: Self) -> Self;

    fn abs(self) -> Self;

    fn copysign(self, sign: Self) -> Self;

    fn sqrt(self) -> Self;

    fn recip_estimate(self) -> Self;

    fn rsqrt_estimate(self) -> Self;

    fn floor(self) -> Self;

    fn ceil(self) -> Self;

    fn round(self) -> Self;

    fn trunc(self) -> Self;

    fn simd_lt(self, other: Self) -> Self::Mask;

    fn simd_le(self, other: Self) -> Self::Mask;

    fn is_nan(self) -> Self::Mask;

    fn reduce_add(self) -> Self::Elem;

    fn reduce_min(self) -> Self::Elem;

    fn reduce_max(self) -> Self::Elem;

    fn prefix_sum(self) -> Self;

    fn to_int_trunc(self) -> Self::Int;

    fn to_int_round(self) -> Self::Int;

    fn from_int(value: Self::Int) -> Self;

    fn load_transposed(rows: &[&[Self::Elem; 4]]) -> [Self; 4];

    fn store_transposed(columns: [Self; 4], rows: &mut [&mut [Self::Elem; 4]]);

    #[inline(always)]
    fn simd_gt(self, other: Self) -> Self::Mask {
        other.simd_lt(self)
    }

    #[inline(always)]
    fn simd_ge(self, other: Self) -> Self::Mask {
        other.simd_le(self)
    }

    #[inline(always)]
    fn clamp(self, lower: Self, upper: Self) -> Self {
        self.max(lower).min(upper)
    }

    #[inline(always)]
    fn is_finite(self) -> Self::Mask {
        self.abs()
            .simd_lt(Self::splat(<Self::Elem as MathElement>::INFINITY))
    }

    #[inline(always)]
    fn to_bits(self) -> Self::Bits {
        self.reinterpret()
    }

    #[inline(always)]
    fn from_bits(bits: Self::Bits) -> Self {
        bits.reinterpret()
    }
}
