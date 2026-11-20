use rikka_physics_math::{Plane, Transform, Vec3};

use super::clip::FeaturePair;
use super::cuboid_capsule::collide_cuboid_capsule;
use super::cuboids::collide_cuboids;
use super::general::collide_general;
use super::hull_round::{SHALLOW, collide_polyhedron_capsule, collide_polyhedron_sphere};
use super::plane::collide_plane;
use super::polyhedra::{Contact, SatCache, collide_polyhedra};
use super::round::{collide_capsule_sphere, collide_capsules, collide_spheres, point};
use super::sat::separating_axis;
use crate::convex::Convex;
use crate::convex::capsule::Capsule;
use crate::convex::cuboid::Cuboid;
use crate::convex::distance::SimplexCache;
use crate::convex::hull::Polyhedron;
use crate::convex::sphere::Sphere;
use crate::convex::support::Support;
use crate::{Manifold, Shape, Tolerance};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PairCache {
    pub simplex: SimplexCache,
    pub sat: SatCache,
}

impl PairCache {
    pub const EMPTY: Self = Self {
        simplex: SimplexCache::EMPTY,
        sat: SatCache::EMPTY,
    };

    #[inline]
    pub fn reset(&mut self) {
        *self = Self::EMPTY;
    }
}

#[derive(Clone, Copy, Debug)]
enum Collider<'a> {
    Convex(Convex<'a>),
    Plane(&'a Plane),
}

impl Collider<'_> {
    fn rank(&self) -> u8 {
        match self {
            Self::Convex(Convex::Sphere(_)) => 0,
            Self::Convex(Convex::Capsule(_)) => 1,
            Self::Convex(Convex::Cuboid(_)) => 2,
            Self::Convex(Convex::Cylinder(_)) => 3,
            Self::Convex(Convex::Hull(_)) => 4,
            Self::Plane(_) => 5,
        }
    }

    fn is_polyhedral(&self) -> bool {
        matches!(self, Self::Convex(Convex::Cuboid(_) | Convex::Hull(_)))
    }
}

fn collider(shape: &Shape) -> Option<Collider<'_>> {
    match shape {
        Shape::Plane(plane) => Some(Collider::Plane(plane)),
        _ => shape.as_convex().map(Collider::Convex),
    }
}

pub fn collide(
    a: &Shape,
    b: &Shape,
    transform: Transform,
    speculative: f32,
    cache: &mut PairCache,
) -> Manifold {
    let (Some(a), Some(b)) = (collider(a), collider(b)) else {
        return Manifold::default();
    };
    if a.rank() >= b.rank() || a.is_polyhedral() && b.is_polyhedral() {
        return collide_ordered(a, b, transform, speculative, cache);
    }
    let flipped = collide_ordered(b, a, transform.inverse(), speculative, cache);
    flip(flipped, transform)
}

fn flip(mut manifold: Manifold, transform: Transform) -> Manifold {
    manifold.normal = -transform.rotate(manifold.normal);
    for index in 0..manifold.count as usize {
        let entry = &mut manifold.points[index];
        entry.point = transform.apply(entry.point);
        entry.feature = FeaturePair::flip_id(entry.feature);
    }
    manifold
}

fn polyhedra(
    a: &Polyhedron<'_>,
    b: &Polyhedron<'_>,
    transform: Transform,
    contact: Contact,
    cache: &mut PairCache,
) -> Manifold {
    collide_polyhedra(a, b, transform, contact, cache, |early_return| {
        separating_axis(
            a,
            b,
            transform,
            contact.threshold(),
            contact.linear_slop,
            early_return,
        )
    })
}

fn shifted(mut manifold: Manifold, offset: f32) -> Manifold {
    if offset == 0.0 {
        return manifold;
    }
    let shift = offset * manifold.normal;
    for index in 0..manifold.count as usize {
        manifold.points[index].point += shift;
    }
    manifold
}

fn polyhedron_sphere(
    hull: &Polyhedron<'_>,
    radius: f32,
    sphere: &Sphere,
    transform: Transform,
    speculative: f32,
    simplex: &mut SimplexCache,
) -> Manifold {
    let inflated = Sphere::with_center(sphere.center, sphere.radius + radius);
    let manifold = collide_polyhedron_sphere(hull, &inflated, transform, speculative, simplex);
    shifted(manifold, radius)
}

fn collide_cuboid_sphere(
    cuboid: &Cuboid,
    sphere: &Sphere,
    transform: Transform,
    speculative: f32,
) -> Manifold {
    let center = transform.apply(sphere.center);
    let local = cuboid.transform.apply_inverse(center);
    let extent = cuboid.half_extents;
    let clamped = local.clamp(-extent, extent);
    let local_offset = local - clamped;
    let distance_squared = local_offset.length_squared();
    let radius = sphere.radius + cuboid.radius;
    let limit = radius + speculative;
    if limit < 0.0 || distance_squared > limit * limit {
        return Manifold::default();
    }
    let surface = cuboid.transform.apply(clamped);
    let (normal, separation) = if distance_squared > SHALLOW * SHALLOW {
        let distance = distance_squared.sqrt();
        (
            cuboid.transform.rotate(local_offset / distance),
            distance - radius,
        )
    } else {
        let mut best_axis = 0;
        let mut best_distance = -f32::MAX;
        let mut best_sign = 1.0f32;
        for axis in 0..3 {
            let value = local[axis];
            let distance = value.abs() - extent[axis];
            if distance > best_distance {
                best_axis = axis;
                best_distance = distance;
                best_sign = if value < 0.0 { -1.0 } else { 1.0 };
            }
        }
        let local_normal = best_sign * Vec3::AXES[best_axis];
        (
            cuboid.transform.rotate(local_normal),
            best_distance - radius,
        )
    };
    let on_hull = center - (center - surface).dot(normal) * normal;
    let deepest = center - radius * normal;
    let mut manifold = Manifold {
        normal,
        ..Manifold::default()
    };
    manifold.push(point(
        on_hull.lerp(deepest, 0.5) + cuboid.radius * normal,
        separation,
        FeaturePair::SINGLE,
    ));
    manifold
}

fn polyhedron_capsule(
    hull: &Polyhedron<'_>,
    radius: f32,
    capsule: &Capsule,
    transform: Transform,
    speculative: f32,
    tolerance: Tolerance,
    simplex: &mut SimplexCache,
) -> Manifold {
    let inflated = Capsule::segment(capsule.center1, capsule.center2, capsule.radius1 + radius);
    let manifold =
        collide_polyhedron_capsule(hull, &inflated, transform, speculative, tolerance, simplex);
    shifted(manifold, radius)
}

fn collide_ordered(
    a: Collider<'_>,
    b: Collider<'_>,
    transform: Transform,
    speculative: f32,
    cache: &mut PairCache,
) -> Manifold {
    let tolerance = Tolerance {
        length_scale: speculative / Tolerance::default().speculative_distance(),
    };
    let (a, b) = match (a, b) {
        (Collider::Plane(plane), Collider::Convex(shape)) => {
            return collide_plane(plane, shape, transform, speculative);
        }
        (Collider::Convex(a), Collider::Convex(b)) => (a, b),
        _ => return Manifold::default(),
    };
    let contact = Contact {
        speculative,
        linear_slop: tolerance.linear_slop(),
        radius_a: a.radius(),
        radius_b: b.radius(),
    };
    match (a, b) {
        (Convex::Sphere(a), Convex::Sphere(b)) => collide_spheres(a, b, transform, speculative),
        (Convex::Capsule(a), Convex::Sphere(b)) => {
            collide_capsule_sphere(a, b, transform, speculative)
        }
        (Convex::Capsule(ca), Convex::Capsule(cb)) if !ca.is_tapered() && !cb.is_tapered() => {
            collide_capsules(ca, cb, transform, speculative, tolerance)
        }
        (Convex::Cuboid(cuboid), Convex::Sphere(sphere)) => {
            collide_cuboid_sphere(cuboid, sphere, transform, speculative)
        }
        (Convex::Cuboid(cuboid), Convex::Capsule(capsule)) if !capsule.is_tapered() => {
            let inflated = Capsule::segment(
                capsule.center1,
                capsule.center2,
                capsule.radius1 + cuboid.radius,
            );
            let manifold = collide_cuboid_capsule(
                cuboid,
                &inflated,
                transform,
                speculative,
                tolerance,
                &mut cache.simplex,
            );
            shifted(manifold, cuboid.radius)
        }
        (Convex::Cuboid(ca), Convex::Cuboid(cb)) if contact.radius() == 0.0 => {
            collide_cuboids(ca, cb, transform, contact, &mut cache.sat)
        }
        (Convex::Cuboid(ca), Convex::Cuboid(cb)) => polyhedra(
            &ca.polyhedron().view(),
            &cb.polyhedron().view(),
            transform,
            contact,
            cache,
        ),
        (Convex::Hull(hull), Convex::Sphere(sphere)) => polyhedron_sphere(
            &hull.polyhedron(),
            hull.radius(),
            sphere,
            transform,
            speculative,
            &mut cache.simplex,
        ),
        (Convex::Hull(hull), Convex::Capsule(capsule)) if !capsule.is_tapered() => {
            polyhedron_capsule(
                &hull.polyhedron(),
                hull.radius(),
                capsule,
                transform,
                speculative,
                tolerance,
                &mut cache.simplex,
            )
        }
        (Convex::Hull(hull), Convex::Cuboid(cuboid)) => polyhedra(
            &hull.polyhedron(),
            &cuboid.polyhedron().view(),
            transform,
            contact,
            cache,
        ),
        (Convex::Cuboid(cuboid), Convex::Hull(hull)) => polyhedra(
            &cuboid.polyhedron().view(),
            &hull.polyhedron(),
            transform,
            contact,
            cache,
        ),
        (Convex::Hull(ha), Convex::Hull(hb)) => polyhedra(
            &ha.polyhedron(),
            &hb.polyhedron(),
            transform,
            contact,
            cache,
        ),
        (a, b) => collide_general(a, b, transform, speculative, &mut cache.simplex),
    }
}
