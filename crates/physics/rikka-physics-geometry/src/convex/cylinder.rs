use std::f32::consts::{PI, TAU};

use rikka_physics_math::{Aabb, Mat3, Transform, Vec3, cos_sin};

use crate::convex::buffer::ProxyBuffer;
use crate::convex::queries;
use crate::convex::revolution::Frustum;
use crate::convex::support::{Support, SupportPoint};
use crate::{CastHit, MassData, RayInput, Result, ShapeCastInput, ShapeGeometry, ShapeProxy};

const PROXY_SIDES: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cylinder {
    pub transform: Transform,
    pub half_height: f32,
    pub radius: f32,
}

impl Cylinder {
    #[inline]
    pub const fn new(half_height: f32, radius: f32) -> Self {
        Self::with_transform(Transform::IDENTITY, half_height, radius)
    }

    #[inline]
    pub const fn with_transform(transform: Transform, half_height: f32, radius: f32) -> Self {
        Self {
            transform,
            half_height,
            radius,
        }
    }

    pub fn validate(&self) -> Result<()> {
        queries::check_transform("cylinder transform", self.transform)?;
        queries::check_positive("cylinder half height", self.half_height)?;
        queries::check_positive("cylinder radius", self.radius)
    }

    #[inline]
    pub fn axis(&self) -> Vec3 {
        self.transform.rotate(Vec3::Y)
    }

    #[inline]
    pub(crate) fn local_support(&self, direction: Vec3) -> Vec3 {
        let radial = Vec3::new(direction.x, 0.0, direction.z);
        let length = radial.length();
        let rim = if length > 1.0e-9 * direction.length().max(f32::MIN_POSITIVE) {
            (self.radius / length) * radial
        } else {
            Vec3::ZERO
        };
        let y = if direction.y >= 0.0 {
            self.half_height
        } else {
            -self.half_height
        };
        Vec3::new(rim.x, y, rim.z)
    }

    pub fn contains(&self, point: Vec3) -> bool {
        let local = self.transform.apply_inverse(point);
        local.y.abs() <= self.half_height
            && local.x * local.x + local.z * local.z <= self.radius * self.radius
    }

    pub fn proxy<'a>(&self, buffer: &'a mut ProxyBuffer) -> ShapeProxy<'a> {
        let radius = self.radius / (PI / PROXY_SIDES as f32).cos();
        let points = (0..2 * PROXY_SIDES).map(|index| {
            let angle = cos_sin(TAU * (index / 2) as f32 / PROXY_SIDES as f32);
            let y = if index % 2 == 0 {
                -self.half_height
            } else {
                self.half_height
            };
            self.transform
                .apply(Vec3::new(radius * angle.cosine, y, radius * angle.sine))
        });
        buffer.fill(points, 0.0)
    }
}

impl Support for Cylinder {
    #[inline]
    fn support(&self, direction: Vec3) -> SupportPoint {
        let local = self.local_support(self.transform.rotate_inverse(direction));
        SupportPoint::continuous(self.transform.apply(local))
    }

    #[inline]
    fn radius(&self) -> f32 {
        0.0
    }

    #[inline]
    fn anchor(&self) -> SupportPoint {
        SupportPoint::continuous(self.transform.apply(Vec3::new(
            self.radius,
            self.half_height,
            0.0,
        )))
    }
}

impl ShapeGeometry for Cylinder {
    fn aabb(&self, transform: Transform) -> Aabb {
        let world = transform * self.transform;
        let axis = world.rotate(Vec3::Y);
        let squared = axis * axis;
        let extent = self.half_height * axis.abs()
            + self.radius * (Vec3::ONE - squared).max(Vec3::ZERO).sqrt();
        Aabb::from_center_extent(world.p, extent)
    }

    fn mass(&self, density: f32) -> MassData {
        let r2 = self.radius * self.radius;
        let height = 2.0 * self.half_height;
        let mass = density * PI * r2 * height;
        let transverse = mass * (3.0 * r2 + height * height) / 12.0;
        let local = Mat3::from_diagonal(Vec3::new(transverse, 0.5 * mass * r2, transverse));
        let rotation = Mat3::from_quat(self.transform.q);
        MassData {
            mass,
            center: self.transform.p,
            inertia: rotation * local * rotation.transpose(),
        }
    }

    fn ray_cast(&self, ray: &RayInput) -> Option<CastHit> {
        if self.contains(ray.origin) {
            return Some(CastHit::new(ray.origin, Vec3::ZERO, 0.0));
        }
        let length = ray.translation.length();
        if length == 0.0 {
            return None;
        }
        let origin = self.transform.apply_inverse(ray.origin);
        let direction = self.transform.rotate_inverse(ray.translation) / length;
        let max_distance = ray.max_fraction * length;
        let mut best: Option<(f32, Vec3)> = None;

        if direction.y != 0.0 {
            for sign in [-1.0f32, 1.0] {
                if sign * direction.y >= 0.0 {
                    continue;
                }
                let t = (sign * self.half_height - origin.y) / direction.y;
                if !(0.0..=max_distance).contains(&t) {
                    continue;
                }
                let p = origin + t * direction;
                if p.x * p.x + p.z * p.z <= self.radius * self.radius
                    && best.is_none_or(|(current, _)| t < current)
                {
                    best = Some((t, Vec3::new(0.0, sign, 0.0)));
                }
            }
        }

        let side = Frustum {
            base: Vec3::ZERO,
            axis: Vec3::Y,
            z0: -self.half_height,
            z1: self.half_height,
            radius0: self.radius,
            radius1: self.radius,
        };
        if let Some((t, normal)) = side.ray(origin, direction, max_distance)
            && best.is_none_or(|(current, _)| t < current)
        {
            best = Some((t, normal));
        }

        best.map(|(t, normal)| {
            let fraction = t / length;
            CastHit::new(
                ray.origin + fraction * ray.translation,
                self.transform.rotate(normal),
                fraction,
            )
        })
    }

    fn shape_cast(&self, cast: &ShapeCastInput<'_>) -> Option<CastHit> {
        queries::shape_cast(self, cast)
    }

    fn overlaps(&self, transform: Transform, proxy: &ShapeProxy<'_>) -> bool {
        queries::overlaps(self, transform, proxy)
    }
}
