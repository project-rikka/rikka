use std::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Mul, MulAssign,
    Neg, Not, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

use crate::Vector;

pub trait Int:
    Vector
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + BitAnd<Output = Self>
    + BitOr<Output = Self>
    + BitXor<Output = Self>
    + Not<Output = Self>
    + Shl<u32, Output = Self>
    + Shr<u32, Output = Self>
    + Shl<Self, Output = Self>
    + Shr<Self, Output = Self>
    + AddAssign
    + SubAssign
    + MulAssign
    + BitAndAssign
    + BitOrAssign
    + BitXorAssign
    + ShlAssign<u32>
    + ShrAssign<u32>
{
    fn simd_lt(self, other: Self) -> Self::Mask;

    fn simd_le(self, other: Self) -> Self::Mask;

    fn min(self, other: Self) -> Self;

    fn max(self, other: Self) -> Self;

    fn saturating_add(self, other: Self) -> Self;

    fn saturating_sub(self, other: Self) -> Self;

    fn count_ones(self) -> Self;

    fn reduce_add(self) -> Self::Elem;

    fn reduce_min(self) -> Self::Elem;

    fn reduce_max(self) -> Self::Elem;

    fn prefix_sum(self) -> Self;

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
}

pub trait SignedInt: Int + Neg<Output = Self> {
    fn abs(self) -> Self;
}
