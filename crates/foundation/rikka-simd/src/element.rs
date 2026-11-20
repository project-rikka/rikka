use std::fmt::Debug;

use crate::math::MathElement;
use crate::sealed::Sealed;

pub(crate) const MAX_LANES: usize = 64;

pub trait Element:
    Copy + Default + Debug + PartialEq + PartialOrd + Send + Sync + 'static + Sealed
{
    #[doc(hidden)]
    type Lane: Lane;

    #[doc(hidden)]
    fn to_lane(self) -> Self::Lane;

    #[doc(hidden)]
    fn from_lane(lane: Self::Lane) -> Self;
}

pub trait Lane: Copy + Default + Debug + Eq + Send + Sync + 'static {
    fn to_u64(self) -> u64;
}

impl Lane for u8 {
    #[inline(always)]
    fn to_u64(self) -> u64 {
        u64::from(self)
    }
}

impl Lane for u16 {
    #[inline(always)]
    fn to_u64(self) -> u64 {
        u64::from(self)
    }
}

impl Lane for u32 {
    #[inline(always)]
    fn to_u64(self) -> u64 {
        u64::from(self)
    }
}

impl Lane for u64 {
    #[inline(always)]
    fn to_u64(self) -> u64 {
        self
    }
}

macro_rules! lane_elements {
    ($($element:ty => $lane:ty),* $(,)?) => {
        $(
            impl Sealed for $element {}

            impl Element for $element {
                type Lane = $lane;

                #[inline(always)]
                fn to_lane(self) -> $lane {
                    self as $lane
                }

                #[inline(always)]
                fn from_lane(lane: $lane) -> Self {
                    lane as $element
                }
            }
        )*
    };
}

lane_elements!(
    u8 => u8, i8 => u8,
    u16 => u16, i16 => u16,
    u32 => u32, i32 => u32,
    u64 => u64, i64 => u64,
);

impl Sealed for f32 {}

impl Element for f32 {
    type Lane = u32;

    #[inline(always)]
    fn to_lane(self) -> u32 {
        self.to_bits()
    }

    #[inline(always)]
    fn from_lane(lane: u32) -> Self {
        f32::from_bits(lane)
    }
}

impl Sealed for f64 {}

impl Element for f64 {
    type Lane = u64;

    #[inline(always)]
    fn to_lane(self) -> u64 {
        self.to_bits()
    }

    #[inline(always)]
    fn from_lane(lane: u64) -> Self {
        f64::from_bits(lane)
    }
}

pub trait FloatElement: Element + MathElement {
    #[doc(hidden)]
    type Bits: Element<Lane = <Self as Element>::Lane>;
    #[doc(hidden)]
    type Int: Element<Lane = <Self as Element>::Lane>;

    #[doc(hidden)]
    fn bits(value: u64) -> Self::Bits;
}

impl FloatElement for f32 {
    type Bits = u32;
    type Int = i32;

    #[inline(always)]
    fn bits(value: u64) -> u32 {
        value as u32
    }
}

impl FloatElement for f64 {
    type Bits = u64;
    type Int = i64;

    #[inline(always)]
    fn bits(value: u64) -> u64 {
        value
    }
}
