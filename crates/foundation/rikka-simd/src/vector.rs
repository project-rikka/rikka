use std::fmt::{self, Debug};

use crate::element::MAX_LANES;
use crate::sealed::Sealed;
use crate::{Element, Mask, Simd};

pub trait Vector: Copy + Debug + Default + PartialEq + Send + Sync + 'static + Sealed {
    type Simd: Simd;
    type Elem: Element;
    type Mask: Mask<Simd = Self::Simd>;
    #[doc(hidden)]
    type Raw: Copy;
    const LANES: usize;

    fn splat(value: Self::Elem) -> Self;

    fn load(slice: &[Self::Elem]) -> Self;

    fn store(self, slice: &mut [Self::Elem]);

    fn load_partial(slice: &[Self::Elem]) -> Self;

    fn store_partial(self, slice: &mut [Self::Elem]);

    fn select(mask: Self::Mask, if_true: Self, if_false: Self) -> Self;

    fn simd_eq(self, other: Self) -> Self::Mask;

    fn reverse(self) -> Self;

    fn interleave(self, other: Self) -> (Self, Self);

    fn deinterleave(self, other: Self) -> (Self, Self);

    #[doc(hidden)]
    fn to_raw(self) -> Self::Raw;

    #[doc(hidden)]
    fn from_raw(raw: Self::Raw) -> Self;

    #[inline(always)]
    fn simd_ne(self, other: Self) -> Self::Mask {
        !self.simd_eq(other)
    }

    #[inline(always)]
    fn from_array<const N: usize>(array: [Self::Elem; N]) -> Self {
        const {
            assert!(
                N == Self::LANES,
                "the array length differs from the lane count"
            )
        };
        Self::load(&array)
    }

    #[inline(always)]
    fn to_array<const N: usize>(self) -> [Self::Elem; N] {
        const {
            assert!(
                N == Self::LANES,
                "the array length differs from the lane count"
            )
        };
        let mut array = [Self::Elem::default(); N];
        self.store(&mut array);
        array
    }

    #[inline(always)]
    fn from_fn(mut f: impl FnMut(usize) -> Self::Elem) -> Self {
        let mut lanes = [Self::Elem::default(); MAX_LANES];
        for (index, lane) in lanes[..Self::LANES].iter_mut().enumerate() {
            *lane = f(index);
        }
        Self::load(&lanes)
    }

    #[inline(always)]
    fn lane(self, index: usize) -> Self::Elem {
        assert!(index < Self::LANES, "lane {index} is out of range");
        let mut lanes = [Self::Elem::default(); MAX_LANES];
        self.store(&mut lanes);
        lanes[index]
    }

    #[inline(always)]
    fn reinterpret<V: Vector<Simd = Self::Simd, Raw = Self::Raw>>(self) -> V {
        V::from_raw(self.to_raw())
    }
}

pub(crate) fn fmt_lanes<V: Vector>(vector: V, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let mut lanes = [V::Elem::default(); MAX_LANES];
    vector.store(&mut lanes);
    f.debug_list().entries(&lanes[..V::LANES]).finish()
}

#[inline(always)]
pub(crate) fn assert_loadable(len: usize, lanes: usize) {
    assert!(
        len >= lanes,
        "a slice of {len} elements is shorter than {lanes} lanes"
    );
}
