use rikka_physics_math::Vec3;

use crate::ShapeProxy;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SupportPoint {
    pub point: Vec3,
    pub id: u32,
}

impl SupportPoint {
    pub const CONTINUOUS: u32 = u32::MAX;

    #[inline]
    pub const fn new(point: Vec3, id: u32) -> Self {
        Self { point, id }
    }

    #[inline]
    pub const fn continuous(point: Vec3) -> Self {
        Self::new(point, Self::CONTINUOUS)
    }

    #[inline]
    pub(crate) fn same_feature(&self, other: &Self) -> bool {
        if self.id == Self::CONTINUOUS || other.id == Self::CONTINUOUS {
            self.point == other.point
        } else {
            self.id == other.id
        }
    }
}

pub trait Support {
    fn support(&self, direction: Vec3) -> SupportPoint;

    fn radius(&self) -> f32;

    #[inline]
    fn anchor(&self) -> SupportPoint {
        self.support(Vec3::X)
    }

    #[inline]
    fn vertex(&self, _id: u32) -> Option<Vec3> {
        None
    }
}

impl<S: Support + ?Sized> Support for &S {
    #[inline]
    fn support(&self, direction: Vec3) -> SupportPoint {
        (**self).support(direction)
    }

    #[inline]
    fn radius(&self) -> f32 {
        (**self).radius()
    }

    #[inline]
    fn anchor(&self) -> SupportPoint {
        (**self).anchor()
    }

    #[inline]
    fn vertex(&self, id: u32) -> Option<Vec3> {
        (**self).vertex(id)
    }
}

impl Support for ShapeProxy<'_> {
    #[inline]
    fn support(&self, direction: Vec3) -> SupportPoint {
        let index = point_support(self.points, direction);
        SupportPoint::new(self.points[index], index as u32)
    }

    #[inline]
    fn radius(&self) -> f32 {
        self.radius
    }

    #[inline]
    fn anchor(&self) -> SupportPoint {
        SupportPoint::new(self.points[0], 0)
    }

    #[inline]
    fn vertex(&self, id: u32) -> Option<Vec3> {
        self.points.get(id as usize).copied()
    }
}

#[inline]
pub(crate) fn point_support(points: &[Vec3], direction: Vec3) -> usize {
    let origin = points[0];
    let mut best = 0;
    let mut best_projection = 0.0;
    for (index, point) in points.iter().enumerate().skip(1) {
        let projection = direction.dot(*point - origin);
        if projection > best_projection {
            best = index;
            best_projection = projection;
        }
    }
    best
}
