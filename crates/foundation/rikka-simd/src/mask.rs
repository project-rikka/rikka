use std::fmt::{self, Debug};
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

use crate::sealed::Sealed;
use crate::{Simd, Vector};

pub trait Mask:
    Copy
    + Debug
    + Default
    + PartialEq
    + Eq
    + Send
    + Sync
    + 'static
    + BitAnd<Output = Self>
    + BitOr<Output = Self>
    + BitXor<Output = Self>
    + Not<Output = Self>
    + BitAndAssign
    + BitOrAssign
    + BitXorAssign
    + Sealed
{
    type Simd: Simd;
    const LANES: usize;

    fn splat(value: bool) -> Self;

    fn from_bitmask(bits: u64) -> Self;

    fn to_bitmask(self) -> u64;

    fn any(self) -> bool;

    fn all(self) -> bool;

    #[inline(always)]
    fn none(self) -> bool {
        !self.any()
    }

    #[inline(always)]
    fn lane(self, index: usize) -> bool {
        assert!(index < Self::LANES, "lane {index} is out of range");
        self.to_bitmask() >> index & 1 != 0
    }

    #[inline(always)]
    fn count(self) -> u32 {
        self.to_bitmask().count_ones()
    }

    #[inline(always)]
    fn first(self) -> Option<usize> {
        let bits = self.to_bitmask();
        (bits != 0).then(|| bits.trailing_zeros() as usize)
    }

    #[inline(always)]
    fn from_fn(mut f: impl FnMut(usize) -> bool) -> Self {
        let mut bits = 0;
        for index in 0..Self::LANES {
            bits |= u64::from(f(index)) << index;
        }
        Self::from_bitmask(bits)
    }

    #[inline(always)]
    fn select<V: Vector<Mask = Self>>(self, if_true: V, if_false: V) -> V {
        V::select(self, if_true, if_false)
    }
}

pub(crate) fn fmt_mask<M: Mask>(mask: M, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let bits = mask.to_bitmask();
    let mut list = f.debug_list();
    for index in 0..M::LANES {
        list.entry(&(bits >> index & 1 != 0));
    }
    list.finish()
}

#[inline(always)]
pub(crate) const fn lane_bits(lanes: usize) -> u64 {
    if lanes >= 64 {
        u64::MAX
    } else {
        (1 << lanes) - 1
    }
}
