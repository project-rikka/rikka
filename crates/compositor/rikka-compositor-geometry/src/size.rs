use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

use crate::space::space_name;
use crate::{Buffer, Coordinate, Logical, Physical, Point, Scale, Transform};

pub struct Size<N, S> {
    pub w: N,
    pub h: N,
    space: PhantomData<S>,
}

impl<N, S> Size<N, S> {
    pub(crate) const fn unchecked(w: N, h: N) -> Self {
        Size {
            w,
            h,
            space: PhantomData,
        }
    }
}

impl<N: Coordinate, S> Size<N, S> {
    pub fn new(w: N, h: N) -> Self {
        debug_assert!(
            w.non_negative() && h.non_negative(),
            "negative size {:?}",
            (w, h)
        );
        Size::unchecked(w, h)
    }

    pub fn zero() -> Self {
        Size::unchecked(N::ZERO, N::ZERO)
    }

    pub fn to_point(self) -> Point<N, S> {
        Point::new(self.w, self.h)
    }

    pub fn to_f64(self) -> Size<f64, S> {
        Size::unchecked(self.w.to_f64(), self.h.to_f64())
    }

    pub fn clamp(self, min: impl Into<Size<N, S>>, max: impl Into<Size<N, S>>) -> Self {
        let min = min.into();
        let max = max.into();
        Size::unchecked(self.w.max(min.w).min(max.w), self.h.max(min.h).min(max.h))
    }

    pub fn upscale(self, scale: impl Into<Scale<N>>) -> Self {
        let scale = scale.into();
        Size::unchecked(self.w.upscale(scale.x), self.h.upscale(scale.y))
    }

    pub fn downscale(self, scale: impl Into<Scale<N>>) -> Self {
        let scale = scale.into();
        Size::unchecked(self.w.downscale(scale.x), self.h.downscale(scale.y))
    }

    pub fn is_empty(&self) -> bool {
        self.w == N::ZERO || self.h == N::ZERO
    }
}

impl<S> Size<f64, S> {
    pub fn to_i32_round<N: Coordinate>(self) -> Size<N, S> {
        Size::unchecked(N::from_f64(self.w.round()), N::from_f64(self.h.round()))
    }

    pub fn to_i32_floor<N: Coordinate>(self) -> Size<N, S> {
        Size::unchecked(N::from_f64(self.w.floor()), N::from_f64(self.h.floor()))
    }

    pub fn to_i32_ceil<N: Coordinate>(self) -> Size<N, S> {
        Size::unchecked(N::from_f64(self.w.ceil()), N::from_f64(self.h.ceil()))
    }
}

impl<N: Coordinate> Size<N, Logical> {
    pub fn to_physical(self, scale: impl Into<Scale<N>>) -> Size<N, Physical> {
        let scale = scale.into();
        Size::unchecked(self.w.upscale(scale.x), self.h.upscale(scale.y))
    }

    pub fn to_physical_precise_round<F: Coordinate, R: Coordinate>(
        self,
        scale: impl Into<Scale<F>>,
    ) -> Size<R, Physical> {
        self.to_f64()
            .to_physical(scale.into().to_f64())
            .to_i32_round()
    }

    pub fn to_physical_precise_ceil<F: Coordinate, R: Coordinate>(
        self,
        scale: impl Into<Scale<F>>,
    ) -> Size<R, Physical> {
        self.to_f64()
            .to_physical(scale.into().to_f64())
            .to_i32_ceil()
    }

    pub fn to_physical_precise_floor<F: Coordinate, R: Coordinate>(
        self,
        scale: impl Into<Scale<F>>,
    ) -> Size<R, Physical> {
        self.to_f64()
            .to_physical(scale.into().to_f64())
            .to_i32_floor()
    }

    pub fn to_buffer(self, scale: impl Into<Scale<N>>, transform: Transform) -> Size<N, Buffer> {
        let scale = scale.into();
        Size::unchecked(self.w.upscale(scale.x), self.h.upscale(scale.y)).transformed(transform)
    }
}

impl<N: Coordinate> Size<N, Physical> {
    pub fn to_logical(self, scale: impl Into<Scale<N>>) -> Size<N, Logical> {
        let scale = scale.into();
        Size::unchecked(self.w.downscale(scale.x), self.h.downscale(scale.y))
    }
}

impl<N: Coordinate> Size<N, Buffer> {
    pub fn to_logical(self, scale: impl Into<Scale<N>>, transform: Transform) -> Size<N, Logical> {
        let scale = scale.into();
        Size::unchecked(self.w.downscale(scale.x), self.h.downscale(scale.y))
            .transformed(transform.inverse())
    }
}

impl<N: Coordinate, S> From<(N, N)> for Size<N, S> {
    fn from((w, h): (N, N)) -> Self {
        Size::new(w, h)
    }
}

impl<N, S> From<Size<N, S>> for (N, N) {
    fn from(size: Size<N, S>) -> Self {
        (size.w, size.h)
    }
}

impl<N: Coordinate, S> Add for Size<N, S> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Size::unchecked(
            self.w.saturating_add(other.w),
            self.h.saturating_add(other.h),
        )
    }
}

impl<N: Coordinate, S> Sub for Size<N, S> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Size::unchecked(
            self.w.saturating_sub(other.w).max(N::ZERO),
            self.h.saturating_sub(other.h).max(N::ZERO),
        )
    }
}

impl<N: Coordinate, S> AddAssign for Size<N, S> {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl<N: Coordinate, S> SubAssign for Size<N, S> {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl<N: Coordinate + Mul<Output = N>, S> Mul<N> for Size<N, S> {
    type Output = Self;

    fn mul(self, factor: N) -> Self {
        Size::unchecked(self.w * factor, self.h * factor)
    }
}

impl<N: Coordinate + Div<Output = N>, S> Div<N> for Size<N, S> {
    type Output = Self;

    fn div(self, divisor: N) -> Self {
        Size::unchecked(self.w / divisor, self.h / divisor)
    }
}

impl<N: Coordinate + Div<Output = N>, S, T> Div<Size<N, T>> for Size<N, S> {
    type Output = Scale<N>;

    fn div(self, other: Size<N, T>) -> Scale<N> {
        Scale {
            x: self.w / other.w,
            y: self.h / other.h,
        }
    }
}

impl<N: Clone, S> Clone for Size<N, S> {
    fn clone(&self) -> Self {
        Size::unchecked(self.w.clone(), self.h.clone())
    }
}

impl<N: Copy, S> Copy for Size<N, S> {}

impl<N: PartialEq, S> PartialEq for Size<N, S> {
    fn eq(&self, other: &Self) -> bool {
        self.w == other.w && self.h == other.h
    }
}

impl<N: Eq, S> Eq for Size<N, S> {}

impl<N: Hash, S> Hash for Size<N, S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.w.hash(state);
        self.h.hash(state);
    }
}

impl<N: Default, S> Default for Size<N, S> {
    fn default() -> Self {
        Size::unchecked(N::default(), N::default())
    }
}

impl<N: fmt::Debug, S> fmt::Debug for Size<N, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(&format!("Size<{}>", space_name::<S>()))
            .field("w", &self.w)
            .field("h", &self.h)
            .finish()
    }
}
