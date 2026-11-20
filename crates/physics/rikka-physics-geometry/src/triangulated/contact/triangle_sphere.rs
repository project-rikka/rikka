use rikka_physics_math::Vec3;

use super::clip::Pair;
use super::local::{ContactFeature, LocalManifold, PointSink};
use crate::triangulated::triangle::closest_point;

pub(crate) fn collide_triangle_sphere(
    triangle: &[Vec3; 3],
    center: Vec3,
    radius: f32,
    speculative: f32,
    sink: &mut PointSink<'_>,
) -> Option<LocalManifold> {
    let [a, b, c] = *triangle;
    let face = (b - a).cross(c - a);
    let offset = face.dot(center - a);
    if offset < 0.0 {
        return None;
    }
    let max_distance = radius + speculative;
    let reach = max_distance * max_distance;
    if offset * offset > reach * face.length_squared() {
        return None;
    }
    let (closest, feature) = closest_point(a, b, c, center);
    let squared_distance = closest.distance_squared(center);
    if squared_distance > reach {
        return None;
    }
    let distance = squared_distance.sqrt();
    let normal = if squared_distance > 1000.0 * f32::MIN_POSITIVE {
        (center - closest) / distance
    } else {
        face.normalize_or_zero()
    };
    let point = 0.5 * (center - radius * normal + closest);
    sink.push(point, distance - radius, Pair::SINGLE.id());
    Some(LocalManifold {
        normal,
        feature: ContactFeature::Triangle(feature),
        squared_distance,
    })
}
