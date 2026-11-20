use rikka_physics_math::{Aabb, Mat3, Transform, Vec3};

use crate::convex::buffer::ProxyBuffer;
use crate::convex::hull::{BOX_CORNERS, BoxPolyhedron, corner_index};
use crate::convex::queries;
use crate::convex::rounding::{ray_rounded, rounded_box_moments};
use crate::convex::support::{Support, SupportPoint};
use crate::{CastHit, MassData, RayInput, Result, ShapeCastInput, ShapeGeometry, ShapeProxy};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cuboid {
    pub transform: Transform,
    pub half_extents: Vec3,
    pub radius: f32,
}

impl Cuboid {
    #[inline]
    pub const fn new(half_extents: Vec3) -> Self {
        Self::with_transform(Transform::IDENTITY, half_extents)
    }

    #[inline]
    pub const fn with_transform(transform: Transform, half_extents: Vec3) -> Self {
        Self {
            transform,
            half_extents,
            radius: 0.0,
        }
    }

    #[inline]
    #[must_use]
    pub const fn rounded(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn validate(&self) -> Result<()> {
        queries::check_transform("cuboid transform", self.transform)?;
        queries::check_positive("cuboid half width", self.half_extents.x)?;
        queries::check_positive("cuboid half height", self.half_extents.y)?;
        queries::check_positive("cuboid half depth", self.half_extents.z)?;
        queries::check_non_negative("cuboid radius", self.radius)
    }

    #[inline]
    pub fn corners(&self) -> [Vec3; 8] {
        BOX_CORNERS.map(|corner| self.transform.apply(Vec3::from(corner) * self.half_extents))
    }

    pub fn proxy<'a>(&self, buffer: &'a mut ProxyBuffer) -> ShapeProxy<'a> {
        buffer.fill(self.corners(), self.radius)
    }

    #[inline]
    pub(crate) fn polyhedron(&self) -> BoxPolyhedron {
        BoxPolyhedron::new(self.transform, self.half_extents)
    }

    #[inline]
    pub(crate) fn axes(&self) -> Mat3 {
        Mat3::from_quat(self.transform.q)
    }
}

#[inline]
pub(crate) fn signs(direction: Vec3) -> Vec3 {
    Vec3::select(direction.cmplt(Vec3::ZERO), Vec3::NEG_ONE, Vec3::ONE)
}

impl Support for Cuboid {
    #[inline]
    fn support(&self, direction: Vec3) -> SupportPoint {
        let signs = signs(self.transform.rotate_inverse(direction));
        SupportPoint::new(
            self.transform.apply(signs * self.half_extents),
            corner_index(signs),
        )
    }

    #[inline]
    fn radius(&self) -> f32 {
        self.radius
    }

    #[inline]
    fn anchor(&self) -> SupportPoint {
        SupportPoint::new(self.transform.apply(self.half_extents), 0)
    }

    #[inline]
    fn vertex(&self, id: u32) -> Option<Vec3> {
        BOX_CORNERS
            .get(id as usize)
            .map(|&corner| self.transform.apply(Vec3::from(corner) * self.half_extents))
    }
}

impl ShapeGeometry for Cuboid {
    fn aabb(&self, transform: Transform) -> Aabb {
        Aabb::from_center_extent(Vec3::ZERO, self.half_extents)
            .transformed(transform * self.transform)
            .inflated(self.radius)
    }

    fn mass(&self, density: f32) -> MassData {
        let (volume, [sx, sy, sz]) = rounded_box_moments(self.half_extents, self.radius);
        let mass = (f64::from(density) * volume) as f32;
        let local = Mat3::from_diagonal(
            Vec3::new((sy + sz) as f32, (sx + sz) as f32, (sx + sy) as f32) * density,
        );
        let rotation = self.axes();
        MassData {
            mass,
            center: self.transform.p,
            inertia: rotation * local * rotation.transpose(),
        }
    }

    fn ray_cast(&self, ray: &RayInput) -> Option<CastHit> {
        if self.radius > 0.0 {
            return ray_rounded(self, self.radius, ray);
        }
        let origin = self.transform.apply_inverse(ray.origin);
        let translation = self.transform.rotate_inverse(ray.translation);
        let h = self.half_extents;
        let mut lower = 0.0f32;
        let mut upper = ray.max_fraction;
        let mut normal = Vec3::ZERO;
        for axis in 0..3 {
            let o = origin[axis];
            let d = translation[axis];
            if d == 0.0 {
                if o < -h[axis] || o > h[axis] {
                    return None;
                }
                continue;
            }
            let inverse = 1.0 / d;
            let (near, far, sign) = if d > 0.0 {
                ((-h[axis] - o) * inverse, (h[axis] - o) * inverse, -1.0)
            } else {
                ((h[axis] - o) * inverse, (-h[axis] - o) * inverse, 1.0)
            };
            if near > lower {
                lower = near;
                normal = Vec3::ZERO;
                normal[axis] = sign;
            }
            upper = upper.min(far);
            if lower > upper {
                return None;
            }
        }
        if normal == Vec3::ZERO {
            return Some(CastHit::new(ray.origin, Vec3::ZERO, 0.0));
        }
        Some(CastHit::new(
            ray.origin + lower * ray.translation,
            self.transform.rotate(normal),
            lower,
        ))
    }

    fn shape_cast(&self, cast: &ShapeCastInput<'_>) -> Option<CastHit> {
        queries::shape_cast(self, cast)
    }

    fn overlaps(&self, transform: Transform, proxy: &ShapeProxy<'_>) -> bool {
        queries::overlaps(self, transform, proxy)
    }
}
