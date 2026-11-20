use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use crate::space::space_name;
use crate::{Buffer, Coordinate, Logical, Physical, Rect, Scale, Size, Transform};

pub struct Point<N, S> {
    pub x: N,
    pub y: N,
    space: PhantomData<S>,
}

impl<N, S> Point<N, S> {
    pub const fn new(x: N, y: N) -> Self {
        Point {
            x,
            y,
            space: PhantomData,
        }
    }
}

impl<N: Coordinate, S> Point<N, S> {
    pub fn zero() -> Self {
        Point::new(N::ZERO, N::ZERO)
    }

    pub fn to_size(self) -> Size<N, S> {
        Size::new(self.x, self.y)
    }

    pub fn to_f64(self) -> Point<f64, S> {
        Point::new(self.x.to_f64(), self.y.to_f64())
    }

    pub fn upscale(self, scale: impl Into<Scale<N>>) -> Self {
        let scale = scale.into();
        Point::new(self.x.upscale(scale.x), self.y.upscale(scale.y))
    }

    pub fn downscale(self, scale: impl Into<Scale<N>>) -> Self {
        let scale = scale.into();
        Point::new(self.x.downscale(scale.x), self.y.downscale(scale.y))
    }

    pub fn constrain(self, rect: impl Into<Rect<N, S>>) -> Self {
        let rect = rect.into();
        Point::new(
            self.x.max(rect.loc.x).min(rect.loc.x + rect.size.w),
            self.y.max(rect.loc.y).min(rect.loc.y + rect.size.h),
        )
    }
}

impl<S> Point<f64, S> {
    pub fn to_i32_round<N: Coordinate>(self) -> Point<N, S> {
        Point::new(N::from_f64(self.x.round()), N::from_f64(self.y.round()))
    }

    pub fn to_i32_floor<N: Coordinate>(self) -> Point<N, S> {
        Point::new(N::from_f64(self.x.floor()), N::from_f64(self.y.floor()))
    }

    pub fn to_i32_ceil<N: Coordinate>(self) -> Point<N, S> {
        Point::new(N::from_f64(self.x.ceil()), N::from_f64(self.y.ceil()))
    }
}

impl<N: Coordinate> Point<N, Logical> {
    pub fn to_physical(self, scale: impl Into<Scale<N>>) -> Point<N, Physical> {
        let scale = scale.into();
        Point::new(self.x.upscale(scale.x), self.y.upscale(scale.y))
    }

    pub fn to_physical_precise_round<F: Coordinate, R: Coordinate>(
        self,
        scale: impl Into<Scale<F>>,
    ) -> Point<R, Physical> {
        self.to_f64()
            .to_physical(scale.into().to_f64())
            .to_i32_round()
    }

    pub fn to_physical_precise_ceil<F: Coordinate, R: Coordinate>(
        self,
        scale: impl Into<Scale<F>>,
    ) -> Point<R, Physical> {
        self.to_f64()
            .to_physical(scale.into().to_f64())
            .to_i32_ceil()
    }

    pub fn to_physical_precise_floor<F: Coordinate, R: Coordinate>(
        self,
        scale: impl Into<Scale<F>>,
    ) -> Point<R, Physical> {
        self.to_f64()
            .to_physical(scale.into().to_f64())
            .to_i32_floor()
    }

    pub fn to_buffer(
        self,
        scale: impl Into<Scale<N>>,
        transform: Transform,
        area: &Size<N, Logical>,
    ) -> Point<N, Buffer> {
        let point = self.transformed_in(transform, area);
        let scale = scale.into();
        Point::new(point.x.upscale(scale.x), point.y.upscale(scale.y))
    }
}

impl<N: Coordinate> Point<N, Physical> {
    pub fn to_logical(self, scale: impl Into<Scale<N>>) -> Point<N, Logical> {
        let scale = scale.into();
        Point::new(self.x.downscale(scale.x), self.y.downscale(scale.y))
    }
}

impl<N: Coordinate> Point<N, Buffer> {
    pub fn to_logical(
        self,
        scale: impl Into<Scale<N>>,
        transform: Transform,
        area: &Size<N, Buffer>,
    ) -> Point<N, Logical> {
        let point = self.transformed_in(transform.inverse(), area);
        let scale = scale.into();
        Point::new(point.x.downscale(scale.x), point.y.downscale(scale.y))
    }
}

impl<N, S> From<(N, N)> for Point<N, S> {
    fn from((x, y): (N, N)) -> Self {
        Point::new(x, y)
    }
}

impl<N, S> From<Point<N, S>> for (N, N) {
    fn from(point: Point<N, S>) -> Self {
        (point.x, point.y)
    }
}

impl<N: Coordinate, S> Add for Point<N, S> {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Point::new(
            self.x.saturating_add(other.x),
            self.y.saturating_add(other.y),
        )
    }
}

impl<N: Coordinate, S> Sub for Point<N, S> {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Point::new(
            self.x.saturating_sub(other.x),
            self.y.saturating_sub(other.y),
        )
    }
}

impl<N: Coordinate, S> AddAssign for Point<N, S> {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl<N: Coordinate, S> SubAssign for Point<N, S> {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl<N: Coordinate, S> Add<Size<N, S>> for Point<N, S> {
    type Output = Self;

    fn add(self, other: Size<N, S>) -> Self {
        Point::new(
            self.x.saturating_add(other.w),
            self.y.saturating_add(other.h),
        )
    }
}

impl<N: Coordinate, S> Sub<Size<N, S>> for Point<N, S> {
    type Output = Self;

    fn sub(self, other: Size<N, S>) -> Self {
        Point::new(
            self.x.saturating_sub(other.w),
            self.y.saturating_sub(other.h),
        )
    }
}

impl<N: Coordinate, S> AddAssign<Size<N, S>> for Point<N, S> {
    fn add_assign(&mut self, other: Size<N, S>) {
        *self = *self + other;
    }
}

impl<N: Coordinate, S> SubAssign<Size<N, S>> for Point<N, S> {
    fn sub_assign(&mut self, other: Size<N, S>) {
        *self = *self - other;
    }
}

impl<N: Clone, S> Clone for Point<N, S> {
    fn clone(&self) -> Self {
        Point::new(self.x.clone(), self.y.clone())
    }
}

impl<N: Copy, S> Copy for Point<N, S> {}

impl<N: PartialEq, S> PartialEq for Point<N, S> {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl<N: Eq, S> Eq for Point<N, S> {}

impl<N: Hash, S> Hash for Point<N, S> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.x.hash(state);
        self.y.hash(state);
    }
}

impl<N: Default, S> Default for Point<N, S> {
    fn default() -> Self {
        Point::new(N::default(), N::default())
    }
}

impl<N: fmt::Debug, S> fmt::Debug for Point<N, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(&format!("Point<{}>", space_name::<S>()))
            .field("x", &self.x)
            .field("y", &self.y)
            .finish()
    }
}
