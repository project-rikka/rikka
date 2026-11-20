use crate::element::FloatElement;
use crate::{Element, Simd};

pub unsafe trait Supports<R> {}

pub trait Register: Copy + Send + Sync + 'static {
    type Mask: Copy + Send + Sync + 'static;
    const BYTES: usize;

    fn zero() -> Self;
    fn ones() -> Self;
    unsafe fn load(ptr: *const u8) -> Self;
    unsafe fn store(self, ptr: *mut u8);
    fn and(self, other: Self) -> Self;
    fn or(self, other: Self) -> Self;
    fn xor(self, other: Self) -> Self;
    fn mask_and(a: Self::Mask, b: Self::Mask) -> Self::Mask;
    fn mask_or(a: Self::Mask, b: Self::Mask) -> Self::Mask;
    fn mask_xor(a: Self::Mask, b: Self::Mask) -> Self::Mask;
    fn mask_any(mask: Self::Mask) -> bool;
}

pub trait LaneOps<R: Register>: Copy + Default + 'static {
    fn mask_not(mask: R::Mask) -> R::Mask;
    fn mask_all(mask: R::Mask) -> bool;
    fn bitmask(mask: R::Mask) -> u64;
    fn from_bitmask(bits: u64) -> R::Mask;
    fn select<S: Simd>(mask: R::Mask, if_true: R, if_false: R) -> R;
    fn splat(value: Self) -> R;
    fn equal<S: Simd>(a: R, b: R) -> R::Mask;
    fn add<S: Simd>(a: R, b: R) -> R;
    fn sub<S: Simd>(a: R, b: R) -> R;
    fn mul<S: Simd>(a: R, b: R) -> R;
    fn shl<S: Simd>(a: R, count: u32) -> R;
    fn shr<S: Simd>(a: R, count: u32) -> R;
    fn shl_each<S: Simd>(a: R, counts: R) -> R;
    fn shr_each<S: Simd>(a: R, counts: R) -> R;
    fn interleave<S: Simd>(a: R, b: R) -> (R, R);
    fn deinterleave<S: Simd>(a: R, b: R) -> (R, R);
    fn reverse<S: Simd>(a: R) -> R;
    unsafe fn load_partial<S: Simd>(ptr: *const Self, len: usize) -> R;
    unsafe fn store_partial<S: Simd>(value: R, ptr: *mut Self, len: usize);
    fn reduce_add<S: Simd>(a: R) -> Self;
    fn prefix_sum<S: Simd>(a: R) -> R;
    fn count_ones<S: Simd>(a: R) -> R;
}

pub trait WideLaneOps<R: Register>: LaneOps<R> {
    unsafe fn gather<S: Simd>(base: *const Self, indices: R) -> R;
    fn compress<S: Simd>(a: R, mask: R::Mask) -> R;
}

pub trait ElemOps<R: Register>: Element<Lane: LaneOps<R>> {
    fn less<S: Simd>(a: R, b: R) -> R::Mask;
    fn less_equal<S: Simd>(a: R, b: R) -> R::Mask;
    fn minimum<S: Simd>(a: R, b: R) -> R;
    fn maximum<S: Simd>(a: R, b: R) -> R;
    fn reduce_min<S: Simd>(a: R) -> Self;
    fn reduce_max<S: Simd>(a: R) -> Self;

    #[inline(always)]
    fn equal<S: Simd>(a: R, b: R) -> R::Mask {
        <Self::Lane as LaneOps<R>>::equal::<S>(a, b)
    }

    #[inline(always)]
    fn add<S: Simd>(a: R, b: R) -> R {
        <Self::Lane as LaneOps<R>>::add::<S>(a, b)
    }

    #[inline(always)]
    fn sub<S: Simd>(a: R, b: R) -> R {
        <Self::Lane as LaneOps<R>>::sub::<S>(a, b)
    }

    #[inline(always)]
    fn mul<S: Simd>(a: R, b: R) -> R {
        <Self::Lane as LaneOps<R>>::mul::<S>(a, b)
    }

    #[inline(always)]
    fn reduce_add<S: Simd>(a: R) -> Self {
        Self::from_lane(<Self::Lane as LaneOps<R>>::reduce_add::<S>(a))
    }

    #[inline(always)]
    fn prefix_sum<S: Simd>(a: R) -> R {
        <Self::Lane as LaneOps<R>>::prefix_sum::<S>(a)
    }
}

pub trait NegOps<R: Register>: ElemOps<R> {
    #[inline(always)]
    fn neg<S: Simd>(a: R) -> R {
        Self::sub::<S>(R::zero(), a)
    }
}

pub trait IntOps<R: Register>: ElemOps<R> {
    fn saturating_add<S: Simd>(a: R, b: R) -> R;
    fn saturating_sub<S: Simd>(a: R, b: R) -> R;

    #[inline(always)]
    fn shr<S: Simd>(a: R, count: u32) -> R {
        <Self::Lane as LaneOps<R>>::shr::<S>(a, count)
    }

    #[inline(always)]
    fn shr_each<S: Simd>(a: R, counts: R) -> R {
        <Self::Lane as LaneOps<R>>::shr_each::<S>(a, counts)
    }
}

pub trait SignedOps<R: Register>: IntOps<R> + NegOps<R> {
    fn abs<S: Simd>(a: R) -> R;
}

pub trait FloatOps<R: Register>:
    ElemOps<R> + NegOps<R> + FloatElement<Bits: IntOps<R>, Int: SignedOps<R>>
{
    fn div<S: Simd>(a: R, b: R) -> R;
    fn sqrt<S: Simd>(a: R) -> R;
    fn recip_estimate<S: Simd>(a: R) -> R;
    fn rsqrt_estimate<S: Simd>(a: R) -> R;
    fn mul_add<S: Simd>(a: R, b: R, c: R) -> R;
    fn mul_sub<S: Simd>(a: R, b: R, c: R) -> R;
    fn neg_mul_add<S: Simd>(a: R, b: R, c: R) -> R;
    fn abs<S: Simd>(a: R) -> R;
    fn copysign<S: Simd>(a: R, sign: R) -> R;
    fn floor<S: Simd>(a: R) -> R;
    fn ceil<S: Simd>(a: R) -> R;
    fn round<S: Simd>(a: R) -> R;
    fn trunc<S: Simd>(a: R) -> R;
    fn is_nan<S: Simd>(a: R) -> R::Mask;
    fn to_int_trunc<S: Simd>(a: R) -> R;
    fn to_int_round<S: Simd>(a: R) -> R;
    fn from_int<S: Simd>(a: R) -> R;
    fn load_transposed<S: Simd>(rows: &[&[Self; 4]]) -> [R; 4];
    fn store_transposed<S: Simd>(columns: [R; 4], rows: &mut [&mut [Self; 4]]);
}

pub trait WidenOps<R: Register>: ElemOps<R> {
    type Wide: ElemOps<R>;

    fn widen_low<S: Simd>(a: R) -> R;
    fn widen_high<S: Simd>(a: R) -> R;
}

pub trait NarrowOps<R: Register>: ElemOps<R> {
    type Narrow: ElemOps<R>;

    fn narrow<S: Simd>(low: R, high: R) -> R;
}

pub trait NarrowUnsignedOps<R: Register>: ElemOps<R> {
    type NarrowUnsigned: ElemOps<R>;

    fn narrow_unsigned<S: Simd>(low: R, high: R) -> R;
}

pub trait GatherOps<R: Register>: ElemOps<R> + Element<Lane: WideLaneOps<R>> {
    type Index: IntOps<R> + Element<Lane = Self::Lane>;

    fn index_limit(len: usize) -> Option<Self::Index>;
    fn index(index: Self::Index) -> usize;
}

pub trait SwizzleOps<R: Register>: ElemOps<R> {
    fn swizzle<S: Simd>(table: R, indices: R) -> R;
}
