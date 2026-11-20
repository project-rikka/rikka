use std::fmt;
use std::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div, DivAssign,
    Mul, MulAssign, Neg, Not, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

use crate::element::MAX_LANES;
use crate::mask::fmt_mask;
use crate::sealed::Sealed;
use crate::vector::{assert_loadable, fmt_lanes};
use crate::{
    Compress, Float, Gather, Int, Mask, Narrow, NarrowUnsigned, SignedInt, Swizzle, Vector, Widen,
};

#[derive(Clone, Copy)]
pub struct Pair<H> {
    low: H,
    high: H,
}

#[derive(Clone, Copy)]
pub struct PairMask<M> {
    low: M,
    high: M,
}

impl<H> Pair<H> {
    #[inline(always)]
    const fn new(low: H, high: H) -> Self {
        Self { low, high }
    }
}

impl<M> PairMask<M> {
    #[inline(always)]
    const fn new(low: M, high: M) -> Self {
        Self { low, high }
    }
}

impl<H> Sealed for Pair<H> {}
impl<M> Sealed for PairMask<M> {}

impl<H: Vector> Vector for Pair<H> {
    type Simd = H::Simd;
    type Elem = H::Elem;
    type Mask = PairMask<H::Mask>;
    type Raw = (H::Raw, H::Raw);
    const LANES: usize = 2 * H::LANES;

    #[inline(always)]
    fn splat(value: H::Elem) -> Self {
        let half = H::splat(value);
        Self::new(half, half)
    }

    #[inline(always)]
    fn load(slice: &[H::Elem]) -> Self {
        assert_loadable(slice.len(), Self::LANES);
        Self::new(H::load(slice), H::load(&slice[H::LANES..]))
    }

    #[inline(always)]
    fn store(self, slice: &mut [H::Elem]) {
        assert_loadable(slice.len(), Self::LANES);
        self.low.store(slice);
        self.high.store(&mut slice[H::LANES..]);
    }

    #[inline(always)]
    fn load_partial(slice: &[H::Elem]) -> Self {
        if slice.len() >= Self::LANES {
            Self::load(slice)
        } else if slice.len() >= H::LANES {
            Self::new(H::load(slice), H::load_partial(&slice[H::LANES..]))
        } else {
            Self::new(H::load_partial(slice), H::default())
        }
    }

    #[inline(always)]
    fn store_partial(self, slice: &mut [H::Elem]) {
        if slice.len() >= H::LANES {
            self.low.store(slice);
            self.high.store_partial(&mut slice[H::LANES..]);
        } else {
            self.low.store_partial(slice);
        }
    }

    #[inline(always)]
    fn select(mask: Self::Mask, if_true: Self, if_false: Self) -> Self {
        Self::new(
            H::select(mask.low, if_true.low, if_false.low),
            H::select(mask.high, if_true.high, if_false.high),
        )
    }

    #[inline(always)]
    fn simd_eq(self, other: Self) -> Self::Mask {
        PairMask::new(self.low.simd_eq(other.low), self.high.simd_eq(other.high))
    }

    #[inline(always)]
    fn reverse(self) -> Self {
        Self::new(self.high.reverse(), self.low.reverse())
    }

    #[inline(always)]
    fn interleave(self, other: Self) -> (Self, Self) {
        let (first, second) = self.low.interleave(other.low);
        let (third, fourth) = self.high.interleave(other.high);
        (Self::new(first, second), Self::new(third, fourth))
    }

    #[inline(always)]
    fn deinterleave(self, other: Self) -> (Self, Self) {
        let (self_even, self_odd) = self.low.deinterleave(self.high);
        let (other_even, other_odd) = other.low.deinterleave(other.high);
        (
            Self::new(self_even, other_even),
            Self::new(self_odd, other_odd),
        )
    }

    #[inline(always)]
    fn to_raw(self) -> Self::Raw {
        (self.low.to_raw(), self.high.to_raw())
    }

    #[inline(always)]
    fn from_raw(raw: Self::Raw) -> Self {
        Self::new(H::from_raw(raw.0), H::from_raw(raw.1))
    }
}

impl<H: Vector> Default for Pair<H> {
    #[inline(always)]
    fn default() -> Self {
        Self::new(H::default(), H::default())
    }
}

impl<H: Vector> PartialEq for Pair<H> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.low == other.low && self.high == other.high
    }
}

impl<H: Vector> fmt::Debug for Pair<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_lanes(*self, f)
    }
}

impl<M: Mask> Mask for PairMask<M> {
    type Simd = M::Simd;
    const LANES: usize = 2 * M::LANES;

    #[inline(always)]
    fn splat(value: bool) -> Self {
        let half = M::splat(value);
        Self::new(half, half)
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> Self {
        Self::new(M::from_bitmask(bits), M::from_bitmask(bits >> M::LANES))
    }

    #[inline(always)]
    fn to_bitmask(self) -> u64 {
        self.low.to_bitmask() | self.high.to_bitmask() << M::LANES
    }

    #[inline(always)]
    fn any(self) -> bool {
        (self.low | self.high).any()
    }

    #[inline(always)]
    fn all(self) -> bool {
        (self.low & self.high).all()
    }
}

impl<M: Mask> Default for PairMask<M> {
    #[inline(always)]
    fn default() -> Self {
        Self::new(M::default(), M::default())
    }
}

impl<M: Mask> PartialEq for PairMask<M> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.low == other.low && self.high == other.high
    }
}

impl<M: Mask> Eq for PairMask<M> {}

impl<M: Mask> fmt::Debug for PairMask<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_mask(*self, f)
    }
}

impl<M: Mask> Not for PairMask<M> {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self {
        Self::new(!self.low, !self.high)
    }
}

macro_rules! mask_operator {
    ($trait:ident, $method:ident, $assign:ident, $assign_method:ident) => {
        impl<M: Mask> $trait for PairMask<M> {
            type Output = Self;

            #[inline(always)]
            fn $method(self, other: Self) -> Self {
                Self::new(self.low.$method(other.low), self.high.$method(other.high))
            }
        }

        impl<M: Mask> $assign for PairMask<M> {
            #[inline(always)]
            fn $assign_method(&mut self, other: Self) {
                *self = $trait::$method(*self, other);
            }
        }
    };
}

mask_operator!(BitAnd, bitand, BitAndAssign, bitand_assign);
mask_operator!(BitOr, bitor, BitOrAssign, bitor_assign);
mask_operator!(BitXor, bitxor, BitXorAssign, bitxor_assign);

macro_rules! operator {
    ($trait:ident, $method:ident, $assign:ident, $assign_method:ident) => {
        impl<H: Vector + $trait<Output = H>> $trait for Pair<H> {
            type Output = Self;

            #[inline(always)]
            fn $method(self, other: Self) -> Self {
                Self::new(self.low.$method(other.low), self.high.$method(other.high))
            }
        }

        impl<H: Vector + $trait<Output = H>> $assign for Pair<H> {
            #[inline(always)]
            fn $assign_method(&mut self, other: Self) {
                *self = $trait::$method(*self, other);
            }
        }
    };
}

operator!(Add, add, AddAssign, add_assign);
operator!(Sub, sub, SubAssign, sub_assign);
operator!(Mul, mul, MulAssign, mul_assign);
operator!(Div, div, DivAssign, div_assign);
operator!(BitAnd, bitand, BitAndAssign, bitand_assign);
operator!(BitOr, bitor, BitOrAssign, bitor_assign);
operator!(BitXor, bitxor, BitXorAssign, bitxor_assign);
operator!(Shl, shl, ShlAssign, shl_assign);
operator!(Shr, shr, ShrAssign, shr_assign);

impl<H: Vector + Neg<Output = H>> Neg for Pair<H> {
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self {
        Self::new(-self.low, -self.high)
    }
}

impl<H: Vector + Not<Output = H>> Not for Pair<H> {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self {
        Self::new(!self.low, !self.high)
    }
}

impl<H: Vector + Shl<u32, Output = H>> Shl<u32> for Pair<H> {
    type Output = Self;

    #[inline(always)]
    fn shl(self, count: u32) -> Self {
        Self::new(self.low << count, self.high << count)
    }
}

impl<H: Vector + Shr<u32, Output = H>> Shr<u32> for Pair<H> {
    type Output = Self;

    #[inline(always)]
    fn shr(self, count: u32) -> Self {
        Self::new(self.low >> count, self.high >> count)
    }
}

impl<H: Vector + Shl<u32, Output = H>> ShlAssign<u32> for Pair<H> {
    #[inline(always)]
    fn shl_assign(&mut self, count: u32) {
        *self = *self << count;
    }
}

impl<H: Vector + Shr<u32, Output = H>> ShrAssign<u32> for Pair<H> {
    #[inline(always)]
    fn shr_assign(&mut self, count: u32) {
        *self = *self >> count;
    }
}

impl<H: Float> Float for Pair<H> {
    type Bits = Pair<H::Bits>;
    type Int = Pair<H::Int>;

    #[inline(always)]
    fn mul_add(self, factor: Self, addend: Self) -> Self {
        Self::new(
            self.low.mul_add(factor.low, addend.low),
            self.high.mul_add(factor.high, addend.high),
        )
    }

    #[inline(always)]
    fn mul_sub(self, factor: Self, subtrahend: Self) -> Self {
        Self::new(
            self.low.mul_sub(factor.low, subtrahend.low),
            self.high.mul_sub(factor.high, subtrahend.high),
        )
    }

    #[inline(always)]
    fn neg_mul_add(self, factor: Self, addend: Self) -> Self {
        Self::new(
            self.low.neg_mul_add(factor.low, addend.low),
            self.high.neg_mul_add(factor.high, addend.high),
        )
    }

    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::new(self.low.min(other.low), self.high.min(other.high))
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::new(self.low.max(other.low), self.high.max(other.high))
    }

    #[inline(always)]
    fn abs(self) -> Self {
        Self::new(self.low.abs(), self.high.abs())
    }

    #[inline(always)]
    fn copysign(self, sign: Self) -> Self {
        Self::new(self.low.copysign(sign.low), self.high.copysign(sign.high))
    }

    #[inline(always)]
    fn sqrt(self) -> Self {
        Self::new(self.low.sqrt(), self.high.sqrt())
    }

    #[inline(always)]
    fn recip_estimate(self) -> Self {
        Self::new(self.low.recip_estimate(), self.high.recip_estimate())
    }

    #[inline(always)]
    fn rsqrt_estimate(self) -> Self {
        Self::new(self.low.rsqrt_estimate(), self.high.rsqrt_estimate())
    }

    #[inline(always)]
    fn floor(self) -> Self {
        Self::new(self.low.floor(), self.high.floor())
    }

    #[inline(always)]
    fn ceil(self) -> Self {
        Self::new(self.low.ceil(), self.high.ceil())
    }

    #[inline(always)]
    fn round(self) -> Self {
        Self::new(self.low.round(), self.high.round())
    }

    #[inline(always)]
    fn trunc(self) -> Self {
        Self::new(self.low.trunc(), self.high.trunc())
    }

    #[inline(always)]
    fn simd_lt(self, other: Self) -> Self::Mask {
        PairMask::new(self.low.simd_lt(other.low), self.high.simd_lt(other.high))
    }

    #[inline(always)]
    fn simd_le(self, other: Self) -> Self::Mask {
        PairMask::new(self.low.simd_le(other.low), self.high.simd_le(other.high))
    }

    #[inline(always)]
    fn is_nan(self) -> Self::Mask {
        PairMask::new(self.low.is_nan(), self.high.is_nan())
    }

    #[inline(always)]
    fn reduce_add(self) -> H::Elem {
        (self.low + self.high).reduce_add()
    }

    #[inline(always)]
    fn reduce_min(self) -> H::Elem {
        self.low.min(self.high).reduce_min()
    }

    #[inline(always)]
    fn reduce_max(self) -> H::Elem {
        self.low.max(self.high).reduce_max()
    }

    #[inline(always)]
    fn prefix_sum(self) -> Self {
        let low = self.low.prefix_sum();
        let carry = H::splat(low.lane(H::LANES - 1));
        Self::new(low, self.high.prefix_sum() + carry)
    }

    #[inline(always)]
    fn to_int_trunc(self) -> Self::Int {
        Pair::new(self.low.to_int_trunc(), self.high.to_int_trunc())
    }

    #[inline(always)]
    fn to_int_round(self) -> Self::Int {
        Pair::new(self.low.to_int_round(), self.high.to_int_round())
    }

    #[inline(always)]
    fn from_int(value: Self::Int) -> Self {
        Self::new(H::from_int(value.low), H::from_int(value.high))
    }

    #[inline(always)]
    fn load_transposed(rows: &[&[H::Elem; 4]]) -> [Self; 4] {
        assert_loadable(rows.len(), Self::LANES);
        let low = H::load_transposed(rows);
        let high = H::load_transposed(&rows[H::LANES..]);
        std::array::from_fn(|column| Self::new(low[column], high[column]))
    }

    #[inline(always)]
    fn store_transposed(columns: [Self; 4], rows: &mut [&mut [H::Elem; 4]]) {
        assert_loadable(rows.len(), Self::LANES);
        let (low, high) = rows.split_at_mut(H::LANES);
        H::store_transposed(columns.map(|column| column.low), low);
        H::store_transposed(columns.map(|column| column.high), high);
    }
}

impl<H: Int> Int for Pair<H> {
    #[inline(always)]
    fn simd_lt(self, other: Self) -> Self::Mask {
        PairMask::new(self.low.simd_lt(other.low), self.high.simd_lt(other.high))
    }

    #[inline(always)]
    fn simd_le(self, other: Self) -> Self::Mask {
        PairMask::new(self.low.simd_le(other.low), self.high.simd_le(other.high))
    }

    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::new(self.low.min(other.low), self.high.min(other.high))
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::new(self.low.max(other.low), self.high.max(other.high))
    }

    #[inline(always)]
    fn saturating_add(self, other: Self) -> Self {
        Self::new(
            self.low.saturating_add(other.low),
            self.high.saturating_add(other.high),
        )
    }

    #[inline(always)]
    fn saturating_sub(self, other: Self) -> Self {
        Self::new(
            self.low.saturating_sub(other.low),
            self.high.saturating_sub(other.high),
        )
    }

    #[inline(always)]
    fn count_ones(self) -> Self {
        Self::new(self.low.count_ones(), self.high.count_ones())
    }

    #[inline(always)]
    fn reduce_add(self) -> H::Elem {
        (self.low + self.high).reduce_add()
    }

    #[inline(always)]
    fn reduce_min(self) -> H::Elem {
        self.low.min(self.high).reduce_min()
    }

    #[inline(always)]
    fn reduce_max(self) -> H::Elem {
        self.low.max(self.high).reduce_max()
    }

    #[inline(always)]
    fn prefix_sum(self) -> Self {
        let low = self.low.prefix_sum();
        let carry = H::splat(low.lane(H::LANES - 1));
        Self::new(low, self.high.prefix_sum() + carry)
    }
}

impl<H: SignedInt> SignedInt for Pair<H> {
    #[inline(always)]
    fn abs(self) -> Self {
        Self::new(self.low.abs(), self.high.abs())
    }
}

impl<H: Widen> Widen for Pair<H> {
    type Wide = Pair<H::Wide>;

    #[inline(always)]
    fn widen_low(self) -> Self::Wide {
        Pair::new(self.low.widen_low(), self.low.widen_high())
    }

    #[inline(always)]
    fn widen_high(self) -> Self::Wide {
        Pair::new(self.high.widen_low(), self.high.widen_high())
    }
}

impl<H: Narrow> Narrow for Pair<H> {
    type Narrow = Pair<H::Narrow>;

    #[inline(always)]
    fn narrow(low: Self, high: Self) -> Self::Narrow {
        Pair::new(H::narrow(low.low, low.high), H::narrow(high.low, high.high))
    }
}

impl<H: NarrowUnsigned> NarrowUnsigned for Pair<H> {
    type NarrowUnsigned = Pair<H::NarrowUnsigned>;

    #[inline(always)]
    fn narrow_unsigned(low: Self, high: Self) -> Self::NarrowUnsigned {
        Pair::new(
            H::narrow_unsigned(low.low, low.high),
            H::narrow_unsigned(high.low, high.high),
        )
    }
}

impl<H: Gather> Gather for Pair<H> {
    type Index = Pair<H::Index>;

    #[inline(always)]
    fn gather(slice: &[H::Elem], indices: Self::Index) -> Self {
        Self::new(
            H::gather(slice, indices.low),
            H::gather(slice, indices.high),
        )
    }
}

impl<H: Compress> Compress for Pair<H> {
    #[inline(always)]
    fn compress(self, mask: Self::Mask) -> Self {
        let mut lanes = [H::Elem::default(); MAX_LANES];
        self.low.compress(mask.low).store(&mut lanes);
        let filled = mask.low.count() as usize;
        self.high.compress(mask.high).store(&mut lanes[filled..]);
        Self::load(&lanes)
    }
}

impl<H: Swizzle + Int<Elem = u8>> Swizzle for Pair<H> {
    #[inline(always)]
    fn swizzle_dyn(self, indices: Self) -> Self {
        let half = H::splat(H::LANES as u8);
        let pick = |index: H| self.low.swizzle_dyn(index) | self.high.swizzle_dyn(index - half);
        Self::new(pick(indices.low), pick(indices.high))
    }
}
