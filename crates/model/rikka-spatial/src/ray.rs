use glam::{BVec3A, Vec3, Vec3A};

use crate::aabb::Aabb;
use crate::plane::Plane;

const SLAB_SCALE: f32 = 1.0 + 2.0 * (3.0 * f32::EPSILON * 0.5) / (1.0 - 3.0 * f32::EPSILON * 0.5);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    origin: Vec3,
    direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Option<Self> {
        let direction = direction.try_normalize()?;
        origin.is_finite().then_some(Self { origin, direction })
    }

    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    pub fn direction(&self) -> Vec3 {
        self.direction
    }

    pub fn at(&self, distance: f32) -> Vec3 {
        self.origin + self.direction * distance
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayInterval {
    pub enter: f32,
    pub exit: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Culling {
    #[default]
    None,
    Back,
    Front,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayTriangleHit {
    pub distance: f32,
    pub barycentric: Vec3,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RaySlab {
    origin: Vec3A,
    inverse: Vec3A,
    positive: BVec3A,
    parallel: BVec3A,
    any_parallel: bool,
}

impl RaySlab {
    pub(crate) fn new(ray: &Ray) -> Self {
        let direction = Vec3A::from(ray.direction);
        let parallel = direction.cmpeq(Vec3A::ZERO);
        let inverse = Vec3A::select(parallel, Vec3A::ZERO, direction.recip());
        Self {
            origin: Vec3A::from(ray.origin),
            inverse,
            positive: direction.cmpge(Vec3A::ZERO),
            parallel,
            any_parallel: parallel.any(),
        }
    }

    #[inline(always)]
    fn bounds(&self, min: Vec3A, max: Vec3A) -> (f32, f32) {
        let near_corner = Vec3A::select(self.positive, min, max);
        let far_corner = Vec3A::select(self.positive, max, min);
        let mut near = (near_corner - self.origin) * self.inverse;
        let mut far = (far_corner - self.origin) * self.inverse;
        if self.any_parallel {
            let inside = self.origin.cmpge(min) & self.origin.cmple(max);
            let unbounded = Vec3A::splat(f32::INFINITY);
            near = Vec3A::select(
                self.parallel,
                Vec3A::select(inside, -unbounded, unbounded),
                near,
            );
            far = Vec3A::select(
                self.parallel,
                Vec3A::select(inside, unbounded, -unbounded),
                far,
            );
        }
        let exit = far.min_element();
        (near.max_element(), exit.max(exit * SLAB_SCALE))
    }

    #[inline(always)]
    pub(crate) fn interval(&self, min: Vec3A, max: Vec3A, near: f32, far: f32) -> Option<f32> {
        let (enter, exit) = self.bounds(min, max);
        let enter = enter.max(near);
        (enter <= exit.min(far)).then_some(enter)
    }

    pub(crate) fn span(&self, min: Vec3A, max: Vec3A, near: f32, far: f32) -> Option<RayInterval> {
        let (enter, exit) = self.bounds(min, max);
        let enter = enter.max(near);
        let exit = exit.min(far);
        (enter <= exit).then_some(RayInterval { enter, exit })
    }
}

pub fn intersect_ray_aabb(ray: &Ray, bounds: &Aabb, max_distance: f32) -> Option<RayInterval> {
    if bounds.is_empty() {
        return None;
    }
    RaySlab::new(ray).span(
        Vec3A::from(bounds.min),
        Vec3A::from(bounds.max),
        0.0,
        max_distance,
    )
}

pub fn intersect_ray_plane(ray: &Ray, plane: &Plane, max_distance: f32) -> Option<f32> {
    let denominator = plane.normal.dot(ray.direction);
    if denominator == 0.0 {
        return None;
    }
    let distance = -plane.signed_distance(ray.origin) / denominator;
    (distance >= 0.0 && distance <= max_distance).then_some(distance)
}

pub fn intersect_ray_sphere(
    ray: &Ray,
    center: Vec3,
    radius: f32,
    max_distance: f32,
) -> Option<RayInterval> {
    let offset = ray.origin - center;
    let half_b = offset.dot(ray.direction);
    let chord = offset - ray.direction * half_b;
    let discriminant = radius * radius - chord.length_squared();
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let c = offset.length_squared() - radius * radius;
    let q = -half_b - root.copysign(half_b);
    let (first, second) = match q == 0.0 {
        true => (0.0, 0.0),
        false => (c / q, q),
    };
    let (near, far) = (first.min(second), first.max(second));
    if far < 0.0 || near > max_distance {
        return None;
    }
    Some(RayInterval {
        enter: near.max(0.0),
        exit: far.min(max_distance),
    })
}

pub fn intersect_ray_triangle(
    ray: &Ray,
    triangle: [Vec3; 3],
    max_distance: f32,
    culling: Culling,
) -> Option<RayTriangleHit> {
    WatertightRay::new(ray).intersect_triangle(triangle, max_distance, culling)
}

#[derive(Debug, Clone, Copy)]
pub struct WatertightRay {
    origin: Vec3,
    axes: [usize; 3],
    shear: Vec3,
}

impl WatertightRay {
    pub fn new(ray: &Ray) -> Self {
        let direction = ray.direction;
        let z = direction.abs().max_position();
        let mut x = (z + 1) % 3;
        let mut y = (x + 1) % 3;
        if direction[z] < 0.0 {
            std::mem::swap(&mut x, &mut y);
        }
        let shear = Vec3::new(
            direction[x] / direction[z],
            direction[y] / direction[z],
            1.0 / direction[z],
        );
        Self {
            origin: ray.origin,
            axes: [x, y, z],
            shear,
        }
    }

    pub fn intersect_triangle(
        &self,
        triangle: [Vec3; 3],
        max_distance: f32,
        culling: Culling,
    ) -> Option<RayTriangleHit> {
        let [kx, ky, kz] = self.axes;
        let [a, b, c] = triangle.map(|vertex| vertex - self.origin);
        let sheared = |vertex: Vec3| {
            (
                vertex[kx] - self.shear.x * vertex[kz],
                vertex[ky] - self.shear.y * vertex[kz],
            )
        };
        let (ax, ay) = sheared(a);
        let (bx, by) = sheared(b);
        let (cx, cy) = sheared(c);
        let mut u = cx * by - cy * bx;
        let mut v = ax * cy - ay * cx;
        let mut w = bx * ay - by * ax;
        if u == 0.0 || v == 0.0 || w == 0.0 {
            let widen = f64::from;
            u = (widen(cx) * widen(by) - widen(cy) * widen(bx)) as f32;
            v = (widen(ax) * widen(cy) - widen(ay) * widen(cx)) as f32;
            w = (widen(bx) * widen(ay) - widen(by) * widen(ax)) as f32;
        }
        let negative = u < 0.0 || v < 0.0 || w < 0.0;
        let positive = u > 0.0 || v > 0.0 || w > 0.0;
        if negative && positive {
            return None;
        }
        let determinant = u + v + w;
        let rejected = match culling {
            Culling::None => determinant == 0.0,
            Culling::Back => determinant <= 0.0,
            Culling::Front => determinant >= 0.0,
        };
        if rejected {
            return None;
        }
        let depth =
            u * (self.shear.z * a[kz]) + v * (self.shear.z * b[kz]) + w * (self.shear.z * c[kz]);
        let sign = determinant.signum();
        let signed_depth = depth * sign;
        let scale = determinant.abs();
        if signed_depth < 0.0 || signed_depth > max_distance * scale {
            return None;
        }
        let inverse = 1.0 / determinant;
        Some(RayTriangleHit {
            distance: depth * inverse,
            barycentric: Vec3::new(u * inverse, v * inverse, w * inverse),
        })
    }
}
