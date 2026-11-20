use rikka_physics_math::{Plane, Transform, Vec3};

use super::clip::{ClipVertex, FeaturePair, Owner, clip_segment};
use crate::convex::capsule::{Capsule, ball_closest};
use crate::convex::distance::{closest_on_segment, segment_distance};
use crate::convex::sphere::Sphere;
use crate::{Manifold, ManifoldPoint, Tolerance};

#[inline(always)]
pub(crate) fn point(point: Vec3, separation: f32, pair: FeaturePair) -> ManifoldPoint {
    ManifoldPoint {
        point,
        separation,
        feature: pair.id(),
        triangle: crate::CastHit::NULL_INDEX,
    }
}

fn single(normal: Vec3, contact: Vec3, separation: f32) -> Manifold {
    let mut manifold = Manifold {
        normal,
        ..Manifold::default()
    };
    manifold.push(point(contact, separation, FeaturePair::SINGLE));
    manifold
}

pub(crate) fn collide_spheres(
    a: &Sphere,
    b: &Sphere,
    transform: Transform,
    speculative: f32,
) -> Manifold {
    let center_b = transform.apply(b.center);
    let surface = ball_closest(a.center, a.radius, center_b, Vec3::Y);
    let separation = surface.distance - b.radius;
    if separation > speculative {
        return Manifold::default();
    }
    let contact = 0.5 * (surface.point + center_b - b.radius * surface.normal);
    single(surface.normal, contact, separation)
}

pub(crate) fn collide_capsule_sphere(
    a: &Capsule,
    b: &Sphere,
    transform: Transform,
    speculative: f32,
) -> Manifold {
    let center_b = transform.apply(b.center);
    let surface = a.closest(center_b);
    let separation = surface.distance - b.radius;
    if separation > speculative {
        return Manifold::default();
    }
    let contact = 0.5 * (surface.point + center_b - b.radius * surface.normal);
    single(surface.normal, contact, separation)
}

pub(crate) fn collide_capsules(
    a: &Capsule,
    b: &Capsule,
    transform: Transform,
    speculative: f32,
    tolerance: Tolerance,
) -> Manifold {
    let a1 = a.center1;
    let a2 = a.center2;
    let b1 = transform.apply(b.center1);
    let b2 = transform.apply(b.center2);
    let radius_a = a.radius1;
    let radius_b = b.radius1;
    let radius = radius_a + radius_b;
    let max_distance = radius + speculative;

    let result = segment_distance(a1, a2, b1, b2);
    let offset = result.point2 - result.point1;
    let distance_squared = offset.length_squared();
    let min_distance = 0.01 * tolerance.linear_slop();
    if distance_squared > max_distance * max_distance
        || distance_squared < min_distance * min_distance
    {
        return Manifold::default();
    }

    let min_length = tolerance.linear_slop();
    let segment_a = a2 - a1;
    let length_a = segment_a.length();
    let segment_b = b2 - b1;
    let length_b = segment_b.length();

    if length_a >= min_length && length_b >= min_length {
        let edge_a = segment_a / length_a;
        let edge_b = segment_b / length_b;
        if edge_a.cross(edge_b).length_squared() < 0.05 * 0.05 {
            let mut clipped = [
                ClipVertex::new(b1, FeaturePair::new(Owner::A, 0, Owner::A, 0)),
                ClipVertex::new(b2, FeaturePair::new(Owner::A, 1, Owner::A, 1)),
            ];
            let mut count = clip_segment(&mut clipped, Plane::new(-edge_a, -edge_a.dot(a1)));
            if count == 2 {
                count = clip_segment(&mut clipped, Plane::new(edge_a, edge_a.dot(a2)));
            }
            if count == 2 {
                let closest1 = closest_on_segment(a1, a2, clipped[0].point());
                let closest2 = closest_on_segment(a1, a2, clipped[1].point());
                let distance1 = closest1.distance(clipped[0].point());
                let distance2 = closest2.distance(clipped[1].point());
                if distance1 <= max_distance
                    && distance2 <= max_distance
                    && distance1 >= min_distance
                    && distance2 >= min_distance
                {
                    let normal1 = (clipped[0].point() - closest1) / distance1;
                    let normal2 = (clipped[1].point() - closest2) / distance2;
                    let normal = (normal1 + normal2).normalize();
                    let mut manifold = Manifold {
                        normal,
                        ..Manifold::default()
                    };
                    for (vertex, closest, local_normal, distance) in [
                        (clipped[0], closest1, normal1, distance1),
                        (clipped[1], closest2, normal2, distance2),
                    ] {
                        let contact = 0.5
                            * (closest + radius_a * local_normal + vertex.point()
                                - radius_b * normal);
                        manifold.push(point(contact, distance - radius, vertex.pair));
                    }
                    return manifold;
                }
            }
        }
    }

    let distance = distance_squared.sqrt();
    let normal = offset / distance;
    let contact = 0.5 * (result.point1 + radius_a * normal + result.point2 - radius_b * normal);
    single(normal, contact, distance - radius)
}
