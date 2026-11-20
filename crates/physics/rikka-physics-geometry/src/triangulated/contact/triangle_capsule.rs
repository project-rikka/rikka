use rikka_physics_math::{Plane, Vec3};

use super::clip::{Pair, TRIANGLE, line_closest, plane_from_points};
use super::local::{ContactFeature, LocalManifold, PlacedTriangle, PointSink};
use crate::triangulated::triangle::TriangleFeature;
use crate::{Capsule, DistanceInput, DistanceOutput, ShapeProxy, SimplexCache, shape_distance};

const PARALLEL_TOLERANCE: f32 = 0.005 * 0.005;

#[derive(Clone, Copy, Debug)]
struct SegmentPoint {
    position: Vec3,
    fraction: f32,
    pair: Pair,
}

#[derive(Clone, Copy, Debug)]
struct Profile {
    start: Vec3,
    end: Vec3,
    radius1: f32,
    radius2: f32,
}

impl Profile {
    #[inline]
    fn radius(&self, fraction: f32) -> f32 {
        self.radius1 + fraction.clamp(0.0, 1.0) * (self.radius2 - self.radius1)
    }

    #[inline]
    fn fraction(&self, point: Vec3) -> f32 {
        let axis = self.end - self.start;
        let length = axis.length_squared();
        if length <= f32::EPSILON {
            0.0
        } else {
            ((point - self.start).dot(axis) / length).clamp(0.0, 1.0)
        }
    }
}

pub(crate) fn triangle_feature(cache: &SimplexCache) -> TriangleFeature {
    let mask = cache.a[..cache.count as usize]
        .iter()
        .fold(0u32, |mask, support| mask | 1 << support.id.min(2));
    match mask {
        0b001 => TriangleFeature::Vertex(0),
        0b010 => TriangleFeature::Vertex(1),
        0b100 => TriangleFeature::Vertex(2),
        0b011 => TriangleFeature::Edge(0),
        0b110 => TriangleFeature::Edge(1),
        0b101 => TriangleFeature::Edge(2),
        _ => TriangleFeature::Face,
    }
}

fn clip_segment(segment: &mut [SegmentPoint; 2], triangle: &[Vec3; 3], normal: Vec3) -> bool {
    let mut vertex1 = triangle[2];
    for &vertex2 in triangle {
        let binormal = (vertex2 - vertex1).normalize_or_zero().cross(normal);
        let plane = Plane::from_normal_point(binormal, vertex1);
        let [p1, p2] = *segment;
        let distance1 = plane.separation(p1.position);
        let distance2 = plane.separation(p2.position);
        let mut clipped = [p1, p2];
        let mut count = 0;
        if distance1 <= 0.0 {
            clipped[count] = p1;
            count += 1;
        }
        if distance2 <= 0.0 {
            clipped[count] = p2;
            count += 1;
        }
        if (distance1 > 0.0) != (distance2 > 0.0) {
            let t = distance1 / (distance1 - distance2);
            clipped[count.min(1)] = SegmentPoint {
                position: p1.position.lerp(p2.position, t),
                fraction: p1.fraction + t * (p2.fraction - p1.fraction),
                pair: if distance1 > 0.0 { p1.pair } else { p2.pair },
            };
            count += 1;
        }
        if count != 2 {
            return false;
        }
        if distance1 > 0.0 && distance2 <= 0.0 {
            clipped.swap(0, 1);
        }
        *segment = clipped;
        vertex1 = vertex2;
    }
    true
}

fn face_contact(
    triangle: &[Vec3; 3],
    plane: Plane,
    profile: &Profile,
    speculative: f32,
    sink: &mut PointSink<'_>,
) -> Option<LocalManifold> {
    let mut segment = [
        SegmentPoint {
            position: profile.start,
            fraction: 0.0,
            pair: Pair::new(TRIANGLE, 0, TRIANGLE, 0),
        },
        SegmentPoint {
            position: profile.end,
            fraction: 1.0,
            pair: Pair::new(TRIANGLE, 1, TRIANGLE, 1),
        },
    ];
    if !clip_segment(&mut segment, triangle, plane.normal) {
        return None;
    }
    let distances = segment.map(|point| plane.separation(point.position));
    let radii = segment.map(|point| profile.radius(point.fraction));
    if distances[0] > speculative + radii[0] && distances[1] > speculative + radii[1] {
        return None;
    }
    for ((point, distance), radius) in segment.iter().zip(distances).zip(radii) {
        let position = point.position - 0.5 * (distance + radius) * plane.normal;
        sink.push(position, distance - radius, point.pair.id());
    }
    Some(LocalManifold {
        normal: plane.normal,
        feature: ContactFeature::Triangle(TriangleFeature::Face),
        squared_distance: 0.0,
    })
}

struct EdgeAxis {
    normal: Vec3,
    separation: f32,
    edge: usize,
}

fn edge_axis(triangle: &[Vec3; 3], plane: Plane, profile: &Profile) -> Option<EdgeAxis> {
    let axis_b = profile.end - profile.start;
    let a = axis_b.dot(plane.normal);
    let mut best: Option<EdgeAxis> = None;
    for edge in 0..3 {
        let v1 = triangle[edge];
        let v2 = triangle[(edge + 1) % 3];
        let side = (v2 - v1).cross(plane.normal).normalize_or_zero();
        let b = axis_b.dot(side);
        if a * a + b * b < PARALLEL_TOLERANCE * axis_b.length_squared() {
            continue;
        }
        let axis = if a * b <= 0.0 {
            side.lerp(plane.normal, b / (b - a))
        } else {
            side.lerp(-plane.normal, b / (a + b))
        };
        let axis = axis.normalize_or_zero();
        let separation = axis.dot(profile.start - v1);
        if best
            .as_ref()
            .is_none_or(|best| separation > best.separation)
        {
            best = Some(EdgeAxis {
                normal: axis,
                separation,
                edge,
            });
        }
    }
    best
}

fn edge_contact(
    triangle: &[Vec3; 3],
    plane: Plane,
    profile: &Profile,
    axis: &EdgeAxis,
    sink: &mut PointSink<'_>,
) -> Option<LocalManifold> {
    let v1 = triangle[axis.edge];
    let v2 = triangle[(axis.edge + 1) % 3];
    let edge = v2 - v1;
    let axis_b = profile.end - profile.start;
    let side = edge.cross(plane.normal).normalize_or_zero();
    let a = axis_b.dot(plane.normal);
    let b = axis_b.dot(side);
    if a * a + b * b < PARALLEL_TOLERANCE * axis_b.length_squared() {
        return None;
    }
    let (point1, fraction1, point2, fraction2) = line_closest(v1, edge, profile.start, axis_b);
    if !(0.0..=1.0).contains(&fraction1) || !(0.0..=1.0).contains(&fraction2) {
        return None;
    }
    let radius = profile.radius(fraction2);
    let point = point1.lerp(point2 - radius * axis.normal, 0.5);
    let separation = axis.normal.dot(profile.start - v1);
    sink.push(
        point,
        separation - radius,
        Pair::new(TRIANGLE, axis.edge as u16, 1, 0).id(),
    );
    Some(LocalManifold {
        normal: axis.normal,
        feature: ContactFeature::Triangle(TriangleFeature::Edge(axis.edge as u8)),
        squared_distance: 0.0,
    })
}

pub(crate) fn collide_triangle_capsule(
    placed: &PlacedTriangle,
    capsule: &Capsule,
    speculative: f32,
    linear_slop: f32,
    cache: &mut SimplexCache,
    sink: &mut PointSink<'_>,
) -> Option<LocalManifold> {
    let triangle = &placed.vertices;
    let [v1, v2, v3] = *triangle;
    let plane = plane_from_points(v1, v2, v3);
    let profile = Profile {
        start: capsule.center1,
        end: capsule.center2,
        radius1: capsule.radius1,
        radius2: capsule.radius2,
    };
    if plane.separation(profile.start.lerp(profile.end, 0.5)) < 0.0 {
        return None;
    }
    let triangle_proxy = ShapeProxy::new(&placed.local, 0.0);
    let ends = [profile.start, profile.end];
    let segment_proxy = ShapeProxy::new(&ends, 0.0);
    let input = DistanceInput::new(&triangle_proxy, &segment_proxy, placed.transform.inverse());
    let local = shape_distance(&input, cache);
    let output = DistanceOutput {
        point_a: placed.transform.apply(local.point_a),
        point_b: placed.transform.apply(local.point_b),
        normal: placed.transform.rotate(local.normal),
        ..local
    };
    let closest_radius = profile.radius(profile.fraction(output.point_b));
    if output.distance > closest_radius + speculative {
        return None;
    }
    if output.distance > 100.0 * f32::EPSILON {
        let delta = (output.point_b - output.point_a).normalize_or_zero();
        if plane.normal.dot(delta).abs() > 0.2
            && let Some(face) = face_contact(triangle, plane, &profile, f32::MAX, sink)
        {
            return Some(LocalManifold {
                squared_distance: output.distance * output.distance,
                ..face
            });
        }
        let point = output
            .point_a
            .lerp(output.point_b - closest_radius * delta, 0.5);
        sink.push(point, output.distance - closest_radius, Pair::SINGLE.id());
        return Some(LocalManifold {
            normal: delta,
            feature: ContactFeature::Triangle(triangle_feature(cache)),
            squared_distance: output.distance * output.distance,
        });
    }
    let face_separation = (plane.separation(profile.start) - profile.radius1)
        .min(plane.separation(profile.end) - profile.radius2);
    if face_separation > 0.0 {
        return None;
    }
    let edge = edge_axis(triangle, plane, &profile);
    let mean_radius = 0.5 * (profile.radius1 + profile.radius2);
    if edge
        .as_ref()
        .is_some_and(|edge| edge.separation > mean_radius)
    {
        return None;
    }
    let face = face_contact(triangle, plane, &profile, speculative, sink);
    let clipped = if face.is_some() {
        sink.min_separation()
    } else {
        face_separation
    };
    let Some(edge) = edge else {
        return face;
    };
    if face.is_none() || edge.separation - mean_radius > clipped + linear_slop {
        sink.clear();
        return edge_contact(triangle, plane, &profile, &edge, sink);
    }
    face
}
