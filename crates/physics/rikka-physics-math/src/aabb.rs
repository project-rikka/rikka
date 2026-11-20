use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::Transform;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub const EMPTY: Self = Self {
        min: Vec3::splat(f32::MAX),
        max: Vec3::splat(-f32::MAX),
    };

    #[inline]
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn from_point(point: Vec3) -> Self {
        Self::new(point, point)
    }

    #[inline]
    pub fn from_center_extent(center: Vec3, extent: Vec3) -> Self {
        Self::new(center - extent, center + extent)
    }

    pub fn from_points(points: impl IntoIterator<Item = Vec3>) -> Self {
        points
            .into_iter()
            .fold(Self::EMPTY, |bounds, point| bounds.including(point))
    }

    #[inline]
    pub fn center(&self) -> Vec3 {
        0.5 * (self.min + self.max)
    }

    #[inline]
    pub fn extent(&self) -> Vec3 {
        0.5 * (self.max - self.min)
    }

    #[inline]
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    #[inline]
    pub fn surface_area(&self) -> f32 {
        let d = self.max - self.min;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.min.is_finite() && self.max.is_finite() && self.min.cmple(self.max).all()
    }

    #[inline]
    pub fn union(&self, other: &Self) -> Self {
        Self::new(self.min.min(other.min), self.max.max(other.max))
    }

    #[inline]
    pub fn including(&self, point: Vec3) -> Self {
        Self::new(self.min.min(point), self.max.max(point))
    }

    #[inline]
    pub fn inflated(&self, margin: f32) -> Self {
        Self::new(
            self.min - Vec3::splat(margin),
            self.max + Vec3::splat(margin),
        )
    }

    #[inline]
    pub fn translated(&self, offset: Vec3) -> Self {
        Self::new(self.min + offset, self.max + offset)
    }

    #[inline]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.min.cmple(other.max).all() && other.min.cmple(self.max).all()
    }

    #[inline]
    pub fn contains(&self, other: &Self) -> bool {
        self.min.cmple(other.min).all() && other.max.cmple(self.max).all()
    }

    #[inline]
    pub fn contains_point(&self, point: Vec3) -> bool {
        self.min.cmple(point).all() && point.cmple(self.max).all()
    }

    pub fn transformed(&self, transform: Transform) -> Self {
        let center = transform.apply(self.center());
        let rotation = glam::Mat3::from_quat(transform.q);
        let extent = self.extent();
        let world_extent = rotation.x_axis.abs() * extent.x
            + rotation.y_axis.abs() * extent.y
            + rotation.z_axis.abs() * extent.z;
        Self::from_center_extent(center, world_extent)
    }

    pub fn ray_fraction(&self, origin: Vec3, translation: Vec3, max_fraction: f32) -> Option<f32> {
        let mut lower = 0.0f32;
        let mut upper = max_fraction;
        for axis in 0..3 {
            let o = origin[axis];
            let d = translation[axis];
            if d.abs() < f32::EPSILON {
                if o < self.min[axis] || o > self.max[axis] {
                    return None;
                }
                continue;
            }
            let inv = 1.0 / d;
            let mut t1 = (self.min[axis] - o) * inv;
            let mut t2 = (self.max[axis] - o) * inv;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            lower = lower.max(t1);
            upper = upper.min(t2);
            if lower > upper {
                return None;
            }
        }
        Some(lower)
    }
}
