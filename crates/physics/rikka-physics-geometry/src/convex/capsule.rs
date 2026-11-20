use rikka_physics_math::{Aabb, Transform, Vec3};

use crate::convex::buffer::ProxyBuffer;
use crate::convex::queries;
use crate::convex::revolution::{Frustum, Revolution};
use crate::convex::sphere::{Sphere, ray_sphere};
use crate::convex::support::{Support, SupportPoint};
use crate::{CastHit, MassData, RayInput, Result, ShapeCastInput, ShapeGeometry, ShapeProxy};

const ICOSAHEDRON_INRADIUS: f32 = 0.794_654_5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capsule {
    pub center1: Vec3,
    pub center2: Vec3,
    pub radius1: f32,
    pub radius2: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Profile {
    pub(crate) small: Vec3,
    pub(crate) big: Vec3,
    pub(crate) small_radius: f32,
    pub(crate) excess: f32,
    pub(crate) small_id: u32,
    pub(crate) big_id: u32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Solid {
    Ball(Sphere),
    Swept {
        axis: Vec3,
        length: f32,
        sine: f32,
        cosine: f32,
    },
}

impl Capsule {
    #[inline]
    pub fn new(radius: f32, half_height: f32) -> Self {
        Self::segment(
            Vec3::new(0.0, -half_height, 0.0),
            Vec3::new(0.0, half_height, 0.0),
            radius,
        )
    }

    #[inline]
    pub const fn segment(center1: Vec3, center2: Vec3, radius: f32) -> Self {
        Self::tapered(center1, radius, center2, radius)
    }

    #[inline]
    pub const fn tapered(center1: Vec3, radius1: f32, center2: Vec3, radius2: f32) -> Self {
        Self {
            center1,
            center2,
            radius1,
            radius2,
        }
    }

    pub fn validate(&self) -> Result<()> {
        queries::check_point("capsule center1", self.center1)?;
        queries::check_point("capsule center2", self.center2)?;
        queries::check_positive("capsule radius1", self.radius1)?;
        queries::check_positive("capsule radius2", self.radius2)
    }

    #[inline]
    pub fn is_tapered(&self) -> bool {
        self.radius1 != self.radius2
    }

    #[inline]
    pub fn max_radius(&self) -> f32 {
        self.radius1.max(self.radius2)
    }

    pub(crate) fn profile(&self) -> Profile {
        if self.radius1 >= self.radius2 {
            Profile {
                small: self.center2,
                big: self.center1,
                small_radius: self.radius2,
                excess: self.radius1 - self.radius2,
                small_id: 1,
                big_id: 0,
            }
        } else {
            Profile {
                small: self.center1,
                big: self.center2,
                small_radius: self.radius1,
                excess: self.radius2 - self.radius1,
                small_id: 0,
                big_id: 1,
            }
        }
    }

    pub(crate) fn solid(&self) -> Solid {
        let delta = self.center2 - self.center1;
        let length = delta.length();
        let difference = self.radius1 - self.radius2;
        if length <= difference.abs() + 1.0e-6 * self.max_radius() {
            return Solid::Ball(if self.radius1 >= self.radius2 {
                Sphere::with_center(self.center1, self.radius1)
            } else {
                Sphere::with_center(self.center2, self.radius2)
            });
        }
        let sine = difference / length;
        Solid::Swept {
            axis: delta / length,
            length,
            sine,
            cosine: (1.0 - sine * sine).max(0.0).sqrt(),
        }
    }

    pub(crate) fn side(&self) -> Option<Frustum> {
        let Solid::Swept {
            axis,
            length,
            sine,
            cosine,
        } = self.solid()
        else {
            return None;
        };
        Some(Frustum {
            base: self.center1,
            axis,
            z0: self.radius1 * sine,
            z1: length + self.radius2 * sine,
            radius0: self.radius1 * cosine,
            radius1: self.radius2 * cosine,
        })
    }

    pub fn contains(&self, point: Vec3) -> bool {
        point.distance_squared(self.center1) <= self.radius1 * self.radius1
            || point.distance_squared(self.center2) <= self.radius2 * self.radius2
            || self.side().is_some_and(|side| side.contains(point))
    }

    pub(crate) fn closest(&self, point: Vec3) -> SurfacePoint {
        match self.solid() {
            Solid::Ball(sphere) => ball_closest(sphere.center, sphere.radius, point, Vec3::Y),
            Solid::Swept {
                axis,
                length,
                sine,
                cosine,
            } => {
                let offset = point - self.center1;
                let z = offset.dot(axis);
                let radial = offset - z * axis;
                let rho = radial.length();
                let outward = if rho > 1.0e-9 * length {
                    radial / rho
                } else {
                    rikka_physics_math::perpendicular(axis)
                };
                let along =
                    (z - self.radius1 * sine) * cosine - (rho - self.radius1 * cosine) * sine;
                if along < 0.0 {
                    ball_closest(self.center1, self.radius1, point, outward)
                } else if along > length * cosine {
                    ball_closest(self.center2, self.radius2, point, outward)
                } else {
                    let normal = sine * axis + cosine * outward;
                    let distance = z * sine + rho * cosine - self.radius1;
                    SurfacePoint {
                        point: point - distance * normal,
                        normal,
                        distance,
                    }
                }
            }
        }
    }

    pub fn proxy<'a>(&self, buffer: &'a mut ProxyBuffer) -> ShapeProxy<'a> {
        let profile = self.profile();
        if profile.excess <= 0.0 {
            return buffer.fill([self.center1, self.center2], self.radius1);
        }
        let scale = profile.excess / ICOSAHEDRON_INRADIUS;
        let points = std::iter::once(profile.small).chain(
            icosahedron()
                .into_iter()
                .map(|vertex| profile.big + scale * vertex),
        );
        buffer.fill(points, profile.small_radius)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SurfacePoint {
    pub(crate) point: Vec3,
    pub(crate) normal: Vec3,
    pub(crate) distance: f32,
}

pub(crate) fn ball_closest(center: Vec3, radius: f32, point: Vec3, fallback: Vec3) -> SurfacePoint {
    let offset = point - center;
    let length = offset.length();
    let normal = if length * length > 1000.0 * f32::MIN_POSITIVE {
        offset / length
    } else {
        fallback
    };
    SurfacePoint {
        point: center + radius * normal,
        normal,
        distance: length - radius,
    }
}

fn icosahedron() -> [Vec3; 12] {
    let phi = 0.5 * (1.0 + 5.0f32.sqrt());
    let n = 1.0 / (1.0 + phi * phi).sqrt();
    let (a, b) = (n, phi * n);
    [
        Vec3::new(0.0, a, b),
        Vec3::new(0.0, a, -b),
        Vec3::new(0.0, -a, b),
        Vec3::new(0.0, -a, -b),
        Vec3::new(a, b, 0.0),
        Vec3::new(a, -b, 0.0),
        Vec3::new(-a, b, 0.0),
        Vec3::new(-a, -b, 0.0),
        Vec3::new(b, 0.0, a),
        Vec3::new(b, 0.0, -a),
        Vec3::new(-b, 0.0, a),
        Vec3::new(-b, 0.0, -a),
    ]
}

impl Support for Capsule {
    #[inline]
    fn support(&self, direction: Vec3) -> SupportPoint {
        let profile = self.profile();
        if profile.excess <= 0.0 {
            return if direction.dot(self.center2 - self.center1) > 0.0 {
                SupportPoint::new(self.center2, 1)
            } else {
                SupportPoint::new(self.center1, 0)
            };
        }
        let length = direction.length();
        if length == 0.0 {
            return SupportPoint::new(profile.big, profile.big_id);
        }
        let big = direction.dot(profile.big) + profile.excess * length;
        if big >= direction.dot(profile.small) {
            SupportPoint::continuous(profile.big + (profile.excess / length) * direction)
        } else {
            SupportPoint::new(profile.small, profile.small_id)
        }
    }

    #[inline]
    fn radius(&self) -> f32 {
        self.radius1.min(self.radius2)
    }

    #[inline]
    fn anchor(&self) -> SupportPoint {
        SupportPoint::new(self.center1, 0)
    }

    #[inline]
    fn vertex(&self, id: u32) -> Option<Vec3> {
        [self.center1, self.center2].get(id as usize).copied()
    }
}

impl ShapeGeometry for Capsule {
    fn aabb(&self, transform: Transform) -> Aabb {
        let a = Aabb::from_center_extent(transform.apply(self.center1), Vec3::splat(self.radius1));
        let b = Aabb::from_center_extent(transform.apply(self.center2), Vec3::splat(self.radius2));
        a.union(&b)
    }

    fn mass(&self, density: f32) -> MassData {
        match self.solid() {
            Solid::Ball(sphere) => sphere.mass(density),
            Solid::Swept {
                axis,
                length,
                sine,
                cosine,
            } => {
                let r1 = f64::from(self.radius1);
                let r2 = f64::from(self.radius2);
                let length = f64::from(length);
                let sine = f64::from(sine);
                let cosine = f64::from(cosine);
                let mut solid = Revolution::default();
                solid.ball_slice(0.0, r1, -r1, r1 * sine);
                solid.frustum(r1 * sine, length + r2 * sine, r1 * cosine, r2 * cosine);
                solid.ball_slice(length, r2, length + r2 * sine, length + r2);
                solid.mass(density, self.center1, axis)
            }
        }
    }

    fn ray_cast(&self, ray: &RayInput) -> Option<CastHit> {
        if self.contains(ray.origin) {
            return Some(CastHit::new(ray.origin, Vec3::ZERO, 0.0));
        }
        let mut best = ray_sphere(self.center1, self.radius1, ray);
        if let Some(hit) = ray_sphere(self.center2, self.radius2, ray)
            && best.is_none_or(|current| hit.fraction < current.fraction)
        {
            best = Some(hit);
        }
        let length = ray.translation.length();
        if length > 0.0
            && let Some(side) = self.side()
            && let Some((distance, normal)) = side.ray(
                ray.origin,
                ray.translation / length,
                ray.max_fraction * length,
            )
        {
            let fraction = distance / length;
            if best.is_none_or(|current| fraction < current.fraction) {
                best = Some(CastHit::new(
                    ray.origin + fraction * ray.translation,
                    normal,
                    fraction,
                ));
            }
        }
        best
    }

    fn shape_cast(&self, cast: &ShapeCastInput<'_>) -> Option<CastHit> {
        queries::shape_cast(self, cast)
    }

    fn overlaps(&self, transform: Transform, proxy: &ShapeProxy<'_>) -> bool {
        queries::overlaps(self, transform, proxy)
    }
}
