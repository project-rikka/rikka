mod ops;
mod r128;
mod r256;
mod r512;

use std::fmt;
use std::marker::PhantomData;
use std::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div, DivAssign,
    Mul, MulAssign, Neg, Not, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

pub(crate) use ops::{
    ElemOps, FloatOps, GatherOps, IntOps, LaneOps, NarrowOps, NarrowUnsignedOps, NegOps, Register,
    SignedOps, Supports, SwizzleOps, WideLaneOps, WidenOps,
};

use crate::element::FloatElement;
use crate::gather::index_out_of_range;
use crate::mask::{fmt_mask, lane_bits};
use crate::sealed::Sealed;
use crate::vector::{assert_loadable, fmt_lanes};
use crate::{
    Compress, Float, Gather, Int, Mask, Narrow, NarrowUnsigned, SignedInt, Simd, Swizzle, Vector,
    Widen,
};

#[repr(transparent)]
pub struct Native<S, E, R> {
    raw: R,
    marker: PhantomData<fn() -> (S, E)>,
}

#[repr(transparent)]
pub struct NativeMask<S, L, R: Register> {
    raw: R::Mask,
    marker: PhantomData<fn() -> (S, L)>,
}

impl<S, E, R: Copy> Clone for Native<S, E, R> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, E, R: Copy> Copy for Native<S, E, R> {}

impl<S, L, R: Register> Clone for NativeMask<S, L, R> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<S, L, R: Register> Copy for NativeMask<S, L, R> {}

impl<S, E, R> Native<S, E, R> {
    #[inline(always)]
    const fn new(raw: R) -> Self {
        Self {
            raw,
            marker: PhantomData,
        }
    }
}

impl<S, L, R: Register> NativeMask<S, L, R> {
    #[inline(always)]
    const fn new(raw: R::Mask) -> Self {
        Self {
            raw,
            marker: PhantomData,
        }
    }
}

impl<S, E, R> Sealed for Native<S, E, R> {}
impl<S, L, R: Register> Sealed for NativeMask<S, L, R> {}

#[inline(always)]
fn shift_count(count: u32) -> std::arch::x86_64::__m128i {
    unsafe { std::arch::x86_64::_mm_cvtsi64_si128(i64::from(count)) }
}

#[inline(always)]
pub(crate) fn to_lanes<R: Copy, L: Copy, const N: usize>(raw: R) -> [L; N] {
    const { assert!(size_of::<R>() == size_of::<[L; N]>()) };
    unsafe { std::mem::transmute_copy(&raw) }
}

#[inline(always)]
pub(crate) fn from_lanes<R: Copy, L: Copy, const N: usize>(lanes: [L; N]) -> R {
    const { assert!(size_of::<R>() == size_of::<[L; N]>()) };
    unsafe { std::mem::transmute_copy(&lanes) }
}

#[inline(always)]
pub(crate) fn map_lanes<R: Copy, L: Copy, const N: usize>(a: R, b: R, f: impl Fn(L, L) -> L) -> R {
    let a: [L; N] = to_lanes(a);
    let b: [L; N] = to_lanes(b);
    from_lanes(std::array::from_fn::<L, N, _>(|lane| f(a[lane], b[lane])))
}

#[inline(always)]
pub(crate) unsafe fn load_prefix<R: Copy, L: Copy + Default, const N: usize>(
    ptr: *const L,
    len: usize,
) -> R {
    let mut lanes = [L::default(); N];
    unsafe { std::ptr::copy_nonoverlapping(ptr, lanes.as_mut_ptr(), len) };
    from_lanes(lanes)
}

#[inline(always)]
pub(crate) unsafe fn store_prefix<R: Copy, L: Copy, const N: usize>(
    value: R,
    ptr: *mut L,
    len: usize,
) {
    let lanes: [L; N] = to_lanes(value);
    unsafe { std::ptr::copy_nonoverlapping(lanes.as_ptr(), ptr, len) };
}

macro_rules! gather_elements {
    ($index:ty, $limit:expr => $($element:ty),*) => {
        $(
            impl<R: Register> GatherOps<R> for $element
            where
                $element: ElemOps<R>,
                $index: IntOps<R> + WideLaneOps<R>,
            {
                type Index = $index;

                #[inline(always)]
                fn index_limit(len: usize) -> Option<$index> {
                    $limit(len)
                }

                #[inline(always)]
                fn index(index: $index) -> usize {
                    index as usize
                }
            }
        )*
    };
}

gather_elements!(u32, |len: usize| (len <= 1 << 31).then_some(len as u32) => f32, i32, u32);
gather_elements!(u64, |len: usize| Some(len as u64) => f64, i64, u64);

impl<S: Simd + Supports<R>, E: ElemOps<R>, R: Register> Vector for Native<S, E, R> {
    type Simd = S;
    type Elem = E;
    type Mask = NativeMask<S, E::Lane, R>;
    type Raw = R;
    const LANES: usize = R::BYTES / size_of::<E>();

    #[inline(always)]
    fn splat(value: E) -> Self {
        Self::new(<E::Lane as LaneOps<R>>::splat(value.to_lane()))
    }

    #[inline(always)]
    fn load(slice: &[E]) -> Self {
        assert_loadable(slice.len(), Self::LANES);
        Self::new(unsafe { R::load(slice.as_ptr().cast()) })
    }

    #[inline(always)]
    fn store(self, slice: &mut [E]) {
        assert_loadable(slice.len(), Self::LANES);
        unsafe { self.raw.store(slice.as_mut_ptr().cast()) }
    }

    #[inline(always)]
    fn load_partial(slice: &[E]) -> Self {
        if slice.len() >= Self::LANES {
            return Self::load(slice);
        }
        Self::new(unsafe {
            <E::Lane as LaneOps<R>>::load_partial::<S>(slice.as_ptr().cast(), slice.len())
        })
    }

    #[inline(always)]
    fn store_partial(self, slice: &mut [E]) {
        if slice.len() >= Self::LANES {
            return self.store(slice);
        }
        unsafe {
            <E::Lane as LaneOps<R>>::store_partial::<S>(
                self.raw,
                slice.as_mut_ptr().cast(),
                slice.len(),
            )
        }
    }

    #[inline(always)]
    fn select(mask: Self::Mask, if_true: Self, if_false: Self) -> Self {
        Self::new(<E::Lane as LaneOps<R>>::select::<S>(
            mask.raw,
            if_true.raw,
            if_false.raw,
        ))
    }

    #[inline(always)]
    fn simd_eq(self, other: Self) -> Self::Mask {
        NativeMask::new(E::equal::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn reverse(self) -> Self {
        Self::new(<E::Lane as LaneOps<R>>::reverse::<S>(self.raw))
    }

    #[inline(always)]
    fn interleave(self, other: Self) -> (Self, Self) {
        let (low, high) = <E::Lane as LaneOps<R>>::interleave::<S>(self.raw, other.raw);
        (Self::new(low), Self::new(high))
    }

    #[inline(always)]
    fn deinterleave(self, other: Self) -> (Self, Self) {
        let (even, odd) = <E::Lane as LaneOps<R>>::deinterleave::<S>(self.raw, other.raw);
        (Self::new(even), Self::new(odd))
    }

    #[inline(always)]
    fn to_raw(self) -> R {
        self.raw
    }

    #[inline(always)]
    fn from_raw(raw: R) -> Self {
        Self::new(raw)
    }
}

impl<S: Simd + Supports<R>, E: ElemOps<R>, R: Register> Default for Native<S, E, R> {
    #[inline(always)]
    fn default() -> Self {
        Self::new(R::zero())
    }
}

impl<S: Simd + Supports<R>, E: ElemOps<R>, R: Register> PartialEq for Native<S, E, R> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.simd_eq(*other).all()
    }
}

impl<S: Simd + Supports<R>, E: ElemOps<R>, R: Register> fmt::Debug for Native<S, E, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_lanes(*self, f)
    }
}

impl<S: Simd + Supports<R>, L: LaneOps<R>, R: Register> Mask for NativeMask<S, L, R> {
    type Simd = S;
    const LANES: usize = R::BYTES / size_of::<L>();

    #[inline(always)]
    fn splat(value: bool) -> Self {
        Self::from_bitmask(if value { u64::MAX } else { 0 })
    }

    #[inline(always)]
    fn from_bitmask(bits: u64) -> Self {
        Self::new(L::from_bitmask(bits & lane_bits(Self::LANES)))
    }

    #[inline(always)]
    fn to_bitmask(self) -> u64 {
        L::bitmask(self.raw)
    }

    #[inline(always)]
    fn any(self) -> bool {
        R::mask_any(self.raw)
    }

    #[inline(always)]
    fn all(self) -> bool {
        L::mask_all(self.raw)
    }
}

impl<S: Simd + Supports<R>, L: LaneOps<R>, R: Register> Default for NativeMask<S, L, R> {
    #[inline(always)]
    fn default() -> Self {
        Self::splat(false)
    }
}

impl<S: Simd + Supports<R>, L: LaneOps<R>, R: Register> PartialEq for NativeMask<S, L, R> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.to_bitmask() == other.to_bitmask()
    }
}

impl<S: Simd + Supports<R>, L: LaneOps<R>, R: Register> Eq for NativeMask<S, L, R> {}

impl<S: Simd + Supports<R>, L: LaneOps<R>, R: Register> fmt::Debug for NativeMask<S, L, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt_mask(*self, f)
    }
}

impl<S: Simd + Supports<R>, L: LaneOps<R>, R: Register> Not for NativeMask<S, L, R> {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self {
        Self::new(L::mask_not(self.raw))
    }
}

macro_rules! mask_operator {
    ($trait:ident, $method:ident, $assign:ident, $assign_method:ident, $op:ident) => {
        impl<S: Simd + Supports<R>, L: LaneOps<R>, R: Register> $trait for NativeMask<S, L, R> {
            type Output = Self;

            #[inline(always)]
            fn $method(self, other: Self) -> Self {
                Self::new(R::$op(self.raw, other.raw))
            }
        }

        impl<S: Simd + Supports<R>, L: LaneOps<R>, R: Register> $assign for NativeMask<S, L, R> {
            #[inline(always)]
            fn $assign_method(&mut self, other: Self) {
                *self = $trait::$method(*self, other);
            }
        }
    };
}

mask_operator!(BitAnd, bitand, BitAndAssign, bitand_assign, mask_and);
mask_operator!(BitOr, bitor, BitOrAssign, bitor_assign, mask_or);
mask_operator!(BitXor, bitxor, BitXorAssign, bitxor_assign, mask_xor);

macro_rules! operator {
    ($bound:ident, $trait:ident, $method:ident, $assign:ident, $assign_method:ident, $op:expr) => {
        impl<S: Simd + Supports<R>, E: $bound<R>, R: Register> $trait for Native<S, E, R> {
            type Output = Self;

            #[inline(always)]
            fn $method(self, other: Self) -> Self {
                Self::new($op(self.raw, other.raw))
            }
        }

        impl<S: Simd + Supports<R>, E: $bound<R>, R: Register> $assign for Native<S, E, R> {
            #[inline(always)]
            fn $assign_method(&mut self, other: Self) {
                *self = $trait::$method(*self, other);
            }
        }
    };
}

operator!(ElemOps, Add, add, AddAssign, add_assign, E::add::<S>);
operator!(ElemOps, Sub, sub, SubAssign, sub_assign, E::sub::<S>);
operator!(ElemOps, Mul, mul, MulAssign, mul_assign, E::mul::<S>);
operator!(FloatOps, Div, div, DivAssign, div_assign, E::div::<S>);
operator!(IntOps, BitAnd, bitand, BitAndAssign, bitand_assign, R::and);
operator!(IntOps, BitOr, bitor, BitOrAssign, bitor_assign, R::or);
operator!(IntOps, BitXor, bitxor, BitXorAssign, bitxor_assign, R::xor);
operator!(
    IntOps,
    Shl,
    shl,
    ShlAssign,
    shl_assign,
    <E::Lane as LaneOps<R>>::shl_each::<S>
);
operator!(IntOps, Shr, shr, ShrAssign, shr_assign, E::shr_each::<S>);

impl<S: Simd + Supports<R>, E: NegOps<R>, R: Register> Neg for Native<S, E, R> {
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self {
        Self::new(E::neg::<S>(self.raw))
    }
}

impl<S: Simd + Supports<R>, E: IntOps<R>, R: Register> Not for Native<S, E, R> {
    type Output = Self;

    #[inline(always)]
    fn not(self) -> Self {
        Self::new(self.raw.xor(R::ones()))
    }
}

impl<S: Simd + Supports<R>, E: IntOps<R>, R: Register> Shl<u32> for Native<S, E, R> {
    type Output = Self;

    #[inline(always)]
    fn shl(self, count: u32) -> Self {
        Self::new(<E::Lane as LaneOps<R>>::shl::<S>(self.raw, count))
    }
}

impl<S: Simd + Supports<R>, E: IntOps<R>, R: Register> Shr<u32> for Native<S, E, R> {
    type Output = Self;

    #[inline(always)]
    fn shr(self, count: u32) -> Self {
        Self::new(E::shr::<S>(self.raw, count))
    }
}

impl<S: Simd + Supports<R>, E: IntOps<R>, R: Register> ShlAssign<u32> for Native<S, E, R> {
    #[inline(always)]
    fn shl_assign(&mut self, count: u32) {
        *self = *self << count;
    }
}

impl<S: Simd + Supports<R>, E: IntOps<R>, R: Register> ShrAssign<u32> for Native<S, E, R> {
    #[inline(always)]
    fn shr_assign(&mut self, count: u32) {
        *self = *self >> count;
    }
}

impl<S: Simd + Supports<R>, E: FloatOps<R>, R: Register> Float for Native<S, E, R> {
    type Bits = Native<S, <E as FloatElement>::Bits, R>;
    type Int = Native<S, <E as FloatElement>::Int, R>;

    #[inline(always)]
    fn mul_add(self, factor: Self, addend: Self) -> Self {
        Self::new(E::mul_add::<S>(self.raw, factor.raw, addend.raw))
    }

    #[inline(always)]
    fn mul_sub(self, factor: Self, subtrahend: Self) -> Self {
        Self::new(E::mul_sub::<S>(self.raw, factor.raw, subtrahend.raw))
    }

    #[inline(always)]
    fn neg_mul_add(self, factor: Self, addend: Self) -> Self {
        Self::new(E::neg_mul_add::<S>(self.raw, factor.raw, addend.raw))
    }

    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::new(E::minimum::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::new(E::maximum::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn abs(self) -> Self {
        Self::new(E::abs::<S>(self.raw))
    }

    #[inline(always)]
    fn copysign(self, sign: Self) -> Self {
        Self::new(E::copysign::<S>(self.raw, sign.raw))
    }

    #[inline(always)]
    fn sqrt(self) -> Self {
        Self::new(E::sqrt::<S>(self.raw))
    }

    #[inline(always)]
    fn recip_estimate(self) -> Self {
        Self::new(E::recip_estimate::<S>(self.raw))
    }

    #[inline(always)]
    fn rsqrt_estimate(self) -> Self {
        Self::new(E::rsqrt_estimate::<S>(self.raw))
    }

    #[inline(always)]
    fn floor(self) -> Self {
        Self::new(E::floor::<S>(self.raw))
    }

    #[inline(always)]
    fn ceil(self) -> Self {
        Self::new(E::ceil::<S>(self.raw))
    }

    #[inline(always)]
    fn round(self) -> Self {
        Self::new(E::round::<S>(self.raw))
    }

    #[inline(always)]
    fn trunc(self) -> Self {
        Self::new(E::trunc::<S>(self.raw))
    }

    #[inline(always)]
    fn simd_lt(self, other: Self) -> Self::Mask {
        NativeMask::new(E::less::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn simd_le(self, other: Self) -> Self::Mask {
        NativeMask::new(E::less_equal::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn is_nan(self) -> Self::Mask {
        NativeMask::new(E::is_nan::<S>(self.raw))
    }

    #[inline(always)]
    fn reduce_add(self) -> E {
        E::reduce_add::<S>(self.raw)
    }

    #[inline(always)]
    fn reduce_min(self) -> E {
        E::reduce_min::<S>(self.raw)
    }

    #[inline(always)]
    fn reduce_max(self) -> E {
        E::reduce_max::<S>(self.raw)
    }

    #[inline(always)]
    fn prefix_sum(self) -> Self {
        Self::new(E::prefix_sum::<S>(self.raw))
    }

    #[inline(always)]
    fn to_int_trunc(self) -> Self::Int {
        Native::new(E::to_int_trunc::<S>(self.raw))
    }

    #[inline(always)]
    fn to_int_round(self) -> Self::Int {
        Native::new(E::to_int_round::<S>(self.raw))
    }

    #[inline(always)]
    fn from_int(value: Self::Int) -> Self {
        Self::new(E::from_int::<S>(value.raw))
    }

    #[inline(always)]
    fn load_transposed(rows: &[&[E; 4]]) -> [Self; 4] {
        assert_loadable(rows.len(), Self::LANES);
        E::load_transposed::<S>(rows).map(Self::new)
    }

    #[inline(always)]
    fn store_transposed(columns: [Self; 4], rows: &mut [&mut [E; 4]]) {
        assert_loadable(rows.len(), Self::LANES);
        E::store_transposed::<S>(columns.map(|column| column.raw), rows)
    }
}

impl<S: Simd + Supports<R>, E: IntOps<R>, R: Register> Int for Native<S, E, R> {
    #[inline(always)]
    fn simd_lt(self, other: Self) -> Self::Mask {
        NativeMask::new(E::less::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn simd_le(self, other: Self) -> Self::Mask {
        NativeMask::new(E::less_equal::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn min(self, other: Self) -> Self {
        Self::new(E::minimum::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        Self::new(E::maximum::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn saturating_add(self, other: Self) -> Self {
        Self::new(E::saturating_add::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn saturating_sub(self, other: Self) -> Self {
        Self::new(E::saturating_sub::<S>(self.raw, other.raw))
    }

    #[inline(always)]
    fn count_ones(self) -> Self {
        Self::new(<E::Lane as LaneOps<R>>::count_ones::<S>(self.raw))
    }

    #[inline(always)]
    fn reduce_add(self) -> E {
        E::reduce_add::<S>(self.raw)
    }

    #[inline(always)]
    fn reduce_min(self) -> E {
        E::reduce_min::<S>(self.raw)
    }

    #[inline(always)]
    fn reduce_max(self) -> E {
        E::reduce_max::<S>(self.raw)
    }

    #[inline(always)]
    fn prefix_sum(self) -> Self {
        Self::new(E::prefix_sum::<S>(self.raw))
    }
}

impl<S: Simd + Supports<R>, E: SignedOps<R>, R: Register> SignedInt for Native<S, E, R> {
    #[inline(always)]
    fn abs(self) -> Self {
        Self::new(E::abs::<S>(self.raw))
    }
}

impl<S: Simd + Supports<R>, E: WidenOps<R>, R: Register> Widen for Native<S, E, R> {
    type Wide = Native<S, E::Wide, R>;

    #[inline(always)]
    fn widen_low(self) -> Self::Wide {
        Native::new(E::widen_low::<S>(self.raw))
    }

    #[inline(always)]
    fn widen_high(self) -> Self::Wide {
        Native::new(E::widen_high::<S>(self.raw))
    }
}

impl<S: Simd + Supports<R>, E: NarrowOps<R>, R: Register> Narrow for Native<S, E, R> {
    type Narrow = Native<S, E::Narrow, R>;

    #[inline(always)]
    fn narrow(low: Self, high: Self) -> Self::Narrow {
        Native::new(E::narrow::<S>(low.raw, high.raw))
    }
}

impl<S: Simd + Supports<R>, E: NarrowUnsignedOps<R>, R: Register> NarrowUnsigned
    for Native<S, E, R>
{
    type NarrowUnsigned = Native<S, E::NarrowUnsigned, R>;

    #[inline(always)]
    fn narrow_unsigned(low: Self, high: Self) -> Self::NarrowUnsigned {
        Native::new(E::narrow_unsigned::<S>(low.raw, high.raw))
    }
}

impl<S: Simd + Supports<R>, E: GatherOps<R>, R: Register> Gather for Native<S, E, R> {
    type Index = Native<S, E::Index, R>;

    #[inline(always)]
    fn gather(slice: &[E], indices: Self::Index) -> Self {
        let Some(limit) = E::index_limit(slice.len()) else {
            return Self::from_fn(|lane| slice[E::index(indices.lane(lane))]);
        };
        if !indices.simd_lt(Self::Index::splat(limit)).all() {
            index_out_of_range(slice.len());
        }
        Self::new(unsafe {
            <E::Lane as WideLaneOps<R>>::gather::<S>(slice.as_ptr().cast(), indices.raw)
        })
    }
}

impl<S: Simd + Supports<R>, E: GatherOps<R>, R: Register> Compress for Native<S, E, R> {
    #[inline(always)]
    fn compress(self, mask: Self::Mask) -> Self {
        Self::new(<E::Lane as WideLaneOps<R>>::compress::<S>(
            self.raw, mask.raw,
        ))
    }
}

impl<S: Simd + Supports<R>, R: Register> Swizzle for Native<S, u8, R>
where
    u8: SwizzleOps<R>,
{
    #[inline(always)]
    fn swizzle_dyn(self, indices: Self) -> Self {
        Self::new(<u8 as SwizzleOps<R>>::swizzle::<S>(self.raw, indices.raw))
    }
}
