use std::f32::consts::PI;

use rikka_physics_math::{Aabb, Mat3, Transform, Vec3};

use crate::convex::queries;
use crate::convex::support::{Support, SupportPoint};
use crate::{CastHit, MassData, RayInput, Result, ShapeCastInput, ShapeGeometry, ShapeProxy};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Sphere {
    #[inline]
    pub const fn new(radius: f32) -> Self {
        Self::with_center(Vec3::ZERO, radius)
    }

    #[inline]
    pub const fn with_center(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn validate(&self) -> Result<()> {
        queries::check_point("sphere center", self.center)?;
        queries::check_positive("sphere radius", self.radius)
    }

    #[inline]
    pub fn proxy(&self) -> ShapeProxy<'_> {
        ShapeProxy::new(std::slice::from_ref(&self.center), self.radius)
    }
}

impl Support for Sphere {
    #[inline]
    fn support(&self, _direction: Vec3) -> SupportPoint {
        SupportPoint::new(self.center, 0)
    }

    #[inline]
    fn radius(&self) -> f32 {
        self.radius
    }

    #[inline]
    fn vertex(&self, id: u32) -> Option<Vec3> {
        (id == 0).then_some(self.center)
    }
}

pub(crate) fn ray_sphere(center: Vec3, radius: f32, ray: &RayInput) -> Option<CastHit> {
    let s = ray.origin - center;
    let rr = radius * radius;
    let length = ray.translation.length();
    if length == 0.0 {
        return (s.length_squared() < rr).then(|| CastHit::new(ray.origin, Vec3::ZERO, 0.0));
    }
    let d = ray.translation / length;
    let t = -s.dot(d);
    let c = s + t * d;
    let cc = c.dot(c);
    if cc > rr {
        return None;
    }
    let fraction = t - (rr - cc).sqrt();
    if fraction < 0.0 || ray.max_fraction * length < fraction {
        return (s.length_squared() < rr).then(|| CastHit::new(ray.origin, Vec3::ZERO, 0.0));
    }
    let normal = (s + fraction * d).normalize();
    Some(CastHit::new(
        center + radius * normal,
        normal,
        (fraction / length).min(ray.max_fraction),
    ))
}

impl ShapeGeometry for Sphere {
    fn aabb(&self, transform: Transform) -> Aabb {
        Aabb::from_center_extent(transform.apply(self.center), Vec3::splat(self.radius))
    }

    fn mass(&self, density: f32) -> MassData {
        let mass = density * 4.0 / 3.0 * PI * self.radius.powi(3);
        MassData {
            mass,
            center: self.center,
            inertia: Mat3::from_diagonal(Vec3::splat(0.4 * mass * self.radius * self.radius)),
        }
    }

    fn ray_cast(&self, ray: &RayInput) -> Option<CastHit> {
        ray_sphere(self.center, self.radius, ray)
    }

    fn shape_cast(&self, cast: &ShapeCastInput<'_>) -> Option<CastHit> {
        queries::shape_cast(self, cast)
    }

    fn overlaps(&self, transform: Transform, proxy: &ShapeProxy<'_>) -> bool {
        queries::overlaps(self, transform, proxy)
    }
}
