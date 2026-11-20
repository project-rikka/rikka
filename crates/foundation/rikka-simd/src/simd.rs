use std::arch::x86_64::{__m128i, __m256i, __m512i};
use std::fmt::Debug;

use crate::native::{Native, NativeMask, Supports};
use crate::pair::{Pair, PairMask};
use crate::sealed::Sealed;
use crate::single::{Single, SingleMask};
use crate::{
    Compress, Float, Gather, Int, Isa, Level, Mask, Narrow, NarrowUnsigned, SignedInt, Swizzle,
    Widen,
};

macro_rules! scalar_types {
    () => {
        type F32x1: Float<
                Simd = Self,
                Elem = f32,
                Mask = Self::M32x1,
                Bits = Self::U32x1,
                Int = Self::I32x1,
                Raw = u32,
            > + Gather<Index = Self::U32x1>
            + Compress;
        type F64x1: Float<
                Simd = Self,
                Elem = f64,
                Mask = Self::M64x1,
                Bits = Self::U64x1,
                Int = Self::I64x1,
                Raw = u64,
            > + Gather<Index = Self::U64x1>
            + Compress;
        type I8x1: SignedInt<Simd = Self, Elem = i8, Mask = Self::M8x1, Raw = u8>;
        type U8x1: Int<Simd = Self, Elem = u8, Mask = Self::M8x1, Raw = u8>;
        type I16x1: SignedInt<Simd = Self, Elem = i16, Mask = Self::M16x1, Raw = u16>;
        type U16x1: Int<Simd = Self, Elem = u16, Mask = Self::M16x1, Raw = u16>;
        type I32x1: SignedInt<Simd = Self, Elem = i32, Mask = Self::M32x1, Raw = u32>
            + Gather<Index = Self::U32x1>
            + Compress;
        type U32x1: Int<Simd = Self, Elem = u32, Mask = Self::M32x1, Raw = u32>
            + Gather<Index = Self::U32x1>
            + Compress;
        type I64x1: SignedInt<Simd = Self, Elem = i64, Mask = Self::M64x1, Raw = u64>
            + Gather<Index = Self::U64x1>
            + Compress;
        type U64x1: Int<Simd = Self, Elem = u64, Mask = Self::M64x1, Raw = u64>
            + Gather<Index = Self::U64x1>
            + Compress;
        type M8x1: Mask<Simd = Self>;
        type M16x1: Mask<Simd = Self>;
        type M32x1: Mask<Simd = Self>;
        type M64x1: Mask<Simd = Self>;
    };
}

macro_rules! width_types {
    ($raw:ident: $f32:ident $f64:ident $i8:ident $u8:ident $i16:ident $u16:ident $i32:ident $u32:ident $i64:ident $u64:ident $m8:ident $m16:ident $m32:ident $m64:ident) => {
        type $f32: Float<
                Simd = Self,
                Elem = f32,
                Mask = Self::$m32,
                Bits = Self::$u32,
                Int = Self::$i32,
                Raw = Self::$raw,
            > + Widen<Wide = Self::$f64>
            + Gather<Index = Self::$u32>
            + Compress;
        type $f64: Float<
                Simd = Self,
                Elem = f64,
                Mask = Self::$m64,
                Bits = Self::$u64,
                Int = Self::$i64,
                Raw = Self::$raw,
            > + Narrow<Narrow = Self::$f32>
            + Gather<Index = Self::$u64>
            + Compress;
        type $i8: SignedInt<Simd = Self, Elem = i8, Mask = Self::$m8, Raw = Self::$raw>
            + Widen<Wide = Self::$i16>;
        type $u8: Int<Simd = Self, Elem = u8, Mask = Self::$m8, Raw = Self::$raw>
            + Widen<Wide = Self::$u16>
            + Swizzle;
        type $i16: SignedInt<Simd = Self, Elem = i16, Mask = Self::$m16, Raw = Self::$raw>
            + Widen<Wide = Self::$i32>
            + Narrow<Narrow = Self::$i8>
            + NarrowUnsigned<NarrowUnsigned = Self::$u8>;
        type $u16: Int<Simd = Self, Elem = u16, Mask = Self::$m16, Raw = Self::$raw>
            + Widen<Wide = Self::$u32>
            + Narrow<Narrow = Self::$u8>;
        type $i32: SignedInt<Simd = Self, Elem = i32, Mask = Self::$m32, Raw = Self::$raw>
            + Widen<Wide = Self::$i64>
            + Narrow<Narrow = Self::$i16>
            + NarrowUnsigned<NarrowUnsigned = Self::$u16>
            + Gather<Index = Self::$u32>
            + Compress;
        type $u32: Int<Simd = Self, Elem = u32, Mask = Self::$m32, Raw = Self::$raw>
            + Widen<Wide = Self::$u64>
            + Narrow<Narrow = Self::$u16>
            + Gather<Index = Self::$u32>
            + Compress;
        type $i64: SignedInt<Simd = Self, Elem = i64, Mask = Self::$m64, Raw = Self::$raw>
            + Narrow<Narrow = Self::$i32>
            + NarrowUnsigned<NarrowUnsigned = Self::$u32>
            + Gather<Index = Self::$u64>
            + Compress;
        type $u64: Int<Simd = Self, Elem = u64, Mask = Self::$m64, Raw = Self::$raw>
            + Narrow<Narrow = Self::$u32>
            + Gather<Index = Self::$u64>
            + Compress;
        type $m8: Mask<Simd = Self>;
        type $m16: Mask<Simd = Self>;
        type $m32: Mask<Simd = Self>;
        type $m64: Mask<Simd = Self>;
    };
}

pub trait Simd: Copy + Debug + Send + Sync + 'static + Sealed {
    const ISA: Isa;

    #[doc(hidden)]
    const SSE42: bool = Self::ISA as u8 >= Isa::Sse42 as u8;
    #[doc(hidden)]
    const AVX2: bool = Self::ISA as u8 >= Isa::Avx2 as u8;
    #[doc(hidden)]
    const AVX512: bool = Self::ISA as u8 >= Isa::Avx512 as u8;

    #[doc(hidden)]
    type Raw128: Copy;
    #[doc(hidden)]
    type Raw256: Copy;
    #[doc(hidden)]
    type Raw512: Copy;
    #[doc(hidden)]
    type RawNative: Copy;

    scalar_types!();
    width_types!(Raw128: F32x4 F64x2 I8x16 U8x16 I16x8 U16x8 I32x4 U32x4 I64x2 U64x2 M8x16 M16x8 M32x4 M64x2);
    width_types!(Raw256: F32x8 F64x4 I8x32 U8x32 I16x16 U16x16 I32x8 U32x8 I64x4 U64x4 M8x32 M16x16 M32x8 M64x4);
    width_types!(Raw512: F32x16 F64x8 I8x64 U8x64 I16x32 U16x32 I32x16 U32x16 I64x8 U64x8 M8x64 M16x32 M32x16 M64x8);
    width_types!(RawNative: F32s F64s I8s U8s I16s U16s I32s U32s I64s U64s M8s M16s M32s M64s);

    fn level(self) -> Level;

    fn vectorize<R>(self, f: impl FnOnce() -> R) -> R;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sse2(());

impl Sse2 {
    pub const fn new() -> Self {
        Self(())
    }
}

impl Default for Sse2 {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sse42(());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Avx2(());

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Avx512(());

impl Sse42 {
    pub(crate) const unsafe fn new_unchecked() -> Self {
        Self(())
    }
}

impl Avx2 {
    pub(crate) const unsafe fn new_unchecked() -> Self {
        Self(())
    }
}

impl Avx512 {
    pub(crate) const unsafe fn new_unchecked() -> Self {
        Self(())
    }
}

unsafe impl Supports<__m256i> for Avx2 {}
unsafe impl Supports<__m256i> for Avx512 {}
unsafe impl Supports<__m512i> for Avx512 {}

macro_rules! scalar_impl {
    ($level:ty) => {
        type F32x1 = Single<$level, f32>;
        type F64x1 = Single<$level, f64>;
        type I8x1 = Single<$level, i8>;
        type U8x1 = Single<$level, u8>;
        type I16x1 = Single<$level, i16>;
        type U16x1 = Single<$level, u16>;
        type I32x1 = Single<$level, i32>;
        type U32x1 = Single<$level, u32>;
        type I64x1 = Single<$level, i64>;
        type U64x1 = Single<$level, u64>;
        type M8x1 = SingleMask<$level, u8>;
        type M16x1 = SingleMask<$level, u16>;
        type M32x1 = SingleMask<$level, u32>;
        type M64x1 = SingleMask<$level, u64>;
    };
}

macro_rules! width_impl {
    ($vector:ident, $mask:ident: $f32:ident $f64:ident $i8:ident $u8:ident $i16:ident $u16:ident $i32:ident $u32:ident $i64:ident $u64:ident $m8:ident $m16:ident $m32:ident $m64:ident) => {
        type $f32 = $vector<f32>;
        type $f64 = $vector<f64>;
        type $i8 = $vector<i8>;
        type $u8 = $vector<u8>;
        type $i16 = $vector<i16>;
        type $u16 = $vector<u16>;
        type $i32 = $vector<i32>;
        type $u32 = $vector<u32>;
        type $i64 = $vector<i64>;
        type $u64 = $vector<u64>;
        type $m8 = $mask<u8>;
        type $m16 = $mask<u16>;
        type $m32 = $mask<u32>;
        type $m64 = $mask<u64>;
    };
}

macro_rules! level_impl {
    (
        $level:ident, $isa:expr, $features:literal,
        raw: $raw128:ty, $raw256:ty, $raw512:ty, $raw_native:ty,
        vectors: $v128:ident, $v256:ident, $v512:ident, $vnative:ident,
        masks: $m128:ident, $m256:ident, $m512:ident, $mnative:ident
    ) => {
        impl Sealed for $level {}

        impl Simd for $level {
            const ISA: Isa = $isa;

            type Raw128 = $raw128;
            type Raw256 = $raw256;
            type Raw512 = $raw512;
            type RawNative = $raw_native;

            scalar_impl!($level);
            width_impl!($v128, $m128: F32x4 F64x2 I8x16 U8x16 I16x8 U16x8 I32x4 U32x4 I64x2 U64x2 M8x16 M16x8 M32x4 M64x2);
            width_impl!($v256, $m256: F32x8 F64x4 I8x32 U8x32 I16x16 U16x16 I32x8 U32x8 I64x4 U64x4 M8x32 M16x16 M32x8 M64x4);
            width_impl!($v512, $m512: F32x16 F64x8 I8x64 U8x64 I16x32 U16x32 I32x16 U32x16 I64x8 U64x8 M8x64 M16x32 M32x16 M64x8);
            width_impl!($vnative, $mnative: F32s F64s I8s U8s I16s U16s I32s U32s I64s U64s M8s M16s M32s M64s);

            #[inline(always)]
            fn level(self) -> Level {
                Level::for_token($isa)
            }

            #[inline(always)]
            fn vectorize<R>(self, f: impl FnOnce() -> R) -> R {
                #[inline]
                #[target_feature(enable = $features)]
                fn run<R>(f: impl FnOnce() -> R) -> R {
                    f()
                }
                unsafe { run(f) }
            }
        }
    };
}

mod sse2 {
    use super::*;

    pub(super) type V128<E> = Native<Sse2, E, __m128i>;
    pub(super) type V256<E> = Pair<V128<E>>;
    pub(super) type V512<E> = Pair<V256<E>>;
    pub(super) type M128<L> = NativeMask<Sse2, L, __m128i>;
    pub(super) type M256<L> = PairMask<M128<L>>;
    pub(super) type M512<L> = PairMask<M256<L>>;
}

mod sse42 {
    use super::*;

    pub(super) type V128<E> = Native<Sse42, E, __m128i>;
    pub(super) type V256<E> = Pair<V128<E>>;
    pub(super) type V512<E> = Pair<V256<E>>;
    pub(super) type M128<L> = NativeMask<Sse42, L, __m128i>;
    pub(super) type M256<L> = PairMask<M128<L>>;
    pub(super) type M512<L> = PairMask<M256<L>>;
}

mod avx2 {
    use super::*;

    pub(super) type V128<E> = Native<Avx2, E, __m128i>;
    pub(super) type V256<E> = Native<Avx2, E, __m256i>;
    pub(super) type V512<E> = Pair<V256<E>>;
    pub(super) type M128<L> = NativeMask<Avx2, L, __m128i>;
    pub(super) type M256<L> = NativeMask<Avx2, L, __m256i>;
    pub(super) type M512<L> = PairMask<M256<L>>;
}

mod avx512 {
    use super::*;

    pub(super) type V128<E> = Native<Avx512, E, __m128i>;
    pub(super) type V256<E> = Native<Avx512, E, __m256i>;
    pub(super) type V512<E> = Native<Avx512, E, __m512i>;
    pub(super) type M128<L> = NativeMask<Avx512, L, __m128i>;
    pub(super) type M256<L> = NativeMask<Avx512, L, __m256i>;
    pub(super) type M512<L> = NativeMask<Avx512, L, __m512i>;
}

impl Sealed for Sse2 {}

impl Simd for Sse2 {
    const ISA: Isa = Isa::Sse2;

    type Raw128 = __m128i;
    type Raw256 = (__m128i, __m128i);
    type Raw512 = ((__m128i, __m128i), (__m128i, __m128i));
    type RawNative = __m128i;

    scalar_impl!(Sse2);
    width_impl!(sse2_v128, sse2_m128: F32x4 F64x2 I8x16 U8x16 I16x8 U16x8 I32x4 U32x4 I64x2 U64x2 M8x16 M16x8 M32x4 M64x2);
    width_impl!(sse2_v256, sse2_m256: F32x8 F64x4 I8x32 U8x32 I16x16 U16x16 I32x8 U32x8 I64x4 U64x4 M8x32 M16x16 M32x8 M64x4);
    width_impl!(sse2_v512, sse2_m512: F32x16 F64x8 I8x64 U8x64 I16x32 U16x32 I32x16 U32x16 I64x8 U64x8 M8x64 M16x32 M32x16 M64x8);
    width_impl!(sse2_v128, sse2_m128: F32s F64s I8s U8s I16s U16s I32s U32s I64s U64s M8s M16s M32s M64s);

    #[inline(always)]
    fn level(self) -> Level {
        Level::baseline()
    }

    #[inline(always)]
    fn vectorize<R>(self, f: impl FnOnce() -> R) -> R {
        f()
    }
}

use avx2::{M128 as avx2_m128, M256 as avx2_m256, M512 as avx2_m512};
use avx2::{V128 as avx2_v128, V256 as avx2_v256, V512 as avx2_v512};
use avx512::{M128 as avx512_m128, M256 as avx512_m256, M512 as avx512_m512};
use avx512::{V128 as avx512_v128, V256 as avx512_v256, V512 as avx512_v512};
use sse2::{M128 as sse2_m128, M256 as sse2_m256, M512 as sse2_m512};
use sse2::{V128 as sse2_v128, V256 as sse2_v256, V512 as sse2_v512};
use sse42::{M128 as sse42_m128, M256 as sse42_m256, M512 as sse42_m512};
use sse42::{V128 as sse42_v128, V256 as sse42_v256, V512 as sse42_v512};

level_impl!(
    Sse42, Isa::Sse42, "sse3,ssse3,sse4.1,sse4.2,popcnt",
    raw: __m128i, (__m128i, __m128i), ((__m128i, __m128i), (__m128i, __m128i)), __m128i,
    vectors: sse42_v128, sse42_v256, sse42_v512, sse42_v128,
    masks: sse42_m128, sse42_m256, sse42_m512, sse42_m128
);

level_impl!(
    Avx2, Isa::Avx2, "sse3,ssse3,sse4.1,sse4.2,popcnt,avx,avx2,fma,bmi1,bmi2,f16c,lzcnt,movbe",
    raw: __m128i, __m256i, (__m256i, __m256i), __m256i,
    vectors: avx2_v128, avx2_v256, avx2_v512, avx2_v256,
    masks: avx2_m128, avx2_m256, avx2_m512, avx2_m256
);

level_impl!(
    Avx512, Isa::Avx512,
    "sse3,ssse3,sse4.1,sse4.2,popcnt,avx,avx2,fma,bmi1,bmi2,f16c,lzcnt,movbe,avx512f,avx512bw,avx512cd,avx512dq,avx512vl",
    raw: __m128i, __m256i, __m512i, __m512i,
    vectors: avx512_v128, avx512_v256, avx512_v512, avx512_v512,
    masks: avx512_m128, avx512_m256, avx512_m512, avx512_m512
);
