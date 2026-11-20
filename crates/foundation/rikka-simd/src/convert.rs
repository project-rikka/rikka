use crate::Vector;

pub trait Widen: Vector {
    type Wide: Vector<Simd = Self::Simd>;

    fn widen_low(self) -> Self::Wide;

    fn widen_high(self) -> Self::Wide;
}

pub trait Narrow: Vector {
    type Narrow: Vector<Simd = Self::Simd>;

    fn narrow(low: Self, high: Self) -> Self::Narrow;
}

pub trait NarrowUnsigned: Vector {
    type NarrowUnsigned: Vector<Simd = Self::Simd>;

    fn narrow_unsigned(low: Self, high: Self) -> Self::NarrowUnsigned;
}
