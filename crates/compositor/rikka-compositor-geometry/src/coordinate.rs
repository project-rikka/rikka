use std::fmt::Debug;
use std::ops::{Add, Sub};

pub trait Coordinate:
    Sized + Add<Output = Self> + Sub<Output = Self> + PartialOrd + Default + Copy + Debug
{
    const ZERO: Self;

    fn downscale(self, scale: Self) -> Self;
    fn upscale(self, scale: Self) -> Self;
    fn to_f64(self) -> f64;
    fn from_f64(value: f64) -> Self;
    fn non_negative(self) -> bool;
    fn abs(self) -> Self;
    fn saturating_add(self, other: Self) -> Self;
    fn saturating_sub(self, other: Self) -> Self;
    fn saturating_mul(self, other: Self) -> Self;

    fn min(self, other: Self) -> Self {
        if self < other { self } else { other }
    }

    fn max(self, other: Self) -> Self {
        if self > other { self } else { other }
    }
}

impl Coordinate for i32 {
    const ZERO: Self = 0;

    fn downscale(self, scale: Self) -> Self {
        self / scale
    }

    fn upscale(self, scale: Self) -> Self {
        i32::saturating_mul(self, scale)
    }

    fn to_f64(self) -> f64 {
        f64::from(self)
    }

    fn from_f64(value: f64) -> Self {
        value as i32
    }

    fn non_negative(self) -> bool {
        self >= 0
    }

    fn abs(self) -> Self {
        i32::abs(self)
    }

    fn saturating_add(self, other: Self) -> Self {
        i32::saturating_add(self, other)
    }

    fn saturating_sub(self, other: Self) -> Self {
        i32::saturating_sub(self, other)
    }

    fn saturating_mul(self, other: Self) -> Self {
        i32::saturating_mul(self, other)
    }
}

impl Coordinate for u32 {
    const ZERO: Self = 0;

    fn downscale(self, scale: Self) -> Self {
        self / scale
    }

    fn upscale(self, scale: Self) -> Self {
        u32::saturating_mul(self, scale)
    }

    fn to_f64(self) -> f64 {
        f64::from(self)
    }

    fn from_f64(value: f64) -> Self {
        value as u32
    }

    fn non_negative(self) -> bool {
        true
    }

    fn abs(self) -> Self {
        self
    }

    fn saturating_add(self, other: Self) -> Self {
        u32::saturating_add(self, other)
    }

    fn saturating_sub(self, other: Self) -> Self {
        u32::saturating_sub(self, other)
    }

    fn saturating_mul(self, other: Self) -> Self {
        u32::saturating_mul(self, other)
    }
}

impl Coordinate for f64 {
    const ZERO: Self = 0.0;

    fn downscale(self, scale: Self) -> Self {
        self / scale
    }

    fn upscale(self, scale: Self) -> Self {
        self * scale
    }

    fn to_f64(self) -> f64 {
        self
    }

    fn from_f64(value: f64) -> Self {
        value
    }

    fn non_negative(self) -> bool {
        self >= 0.0
    }

    fn abs(self) -> Self {
        f64::abs(self)
    }

    fn saturating_add(self, other: Self) -> Self {
        self + other
    }

    fn saturating_sub(self, other: Self) -> Self {
        self - other
    }

    fn saturating_mul(self, other: Self) -> Self {
        self * other
    }
}
