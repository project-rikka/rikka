use rikka_physics_math::{Plane, Transform, Vec3};

use super::clip::{ClipVertex, FeaturePair, MAX_CLIP_POINTS, Owner, clip_polygon, clip_segment};
use super::faces::{FLAT, Face, supporting_face};
use super::reduce::reduce;
use super::round::point;
use crate::convex::Convex;
use crate::convex::distance::{SimplexCache, closest_features, closest_on_segment};
use crate::{Manifold, ManifoldPoint};

const PARALLEL: f32 = 0.05;

struct Candidates {
    points: [ManifoldPoint; MAX_CLIP_POINTS],
    count: usize,
}

impl Candidates {
    fn push(&mut self, contact: Vec3, separation: f32, pair: FeaturePair, speculative: f32) {
        if separation <= speculative && self.count < MAX_CLIP_POINTS {
            self.points[self.count] = point(contact, separation, pair);
            self.count += 1;
        }
    }
}

fn newell(points: &[Vec3]) -> Vec3 {
    let mut normal = Vec3::ZERO;
    let origin = points[0];
    for (index, &current) in points.iter().enumerate() {
        let v1 = current - origin;
        let v2 = points[(index + 1) % points.len()] - origin;
        normal.x += (v1.y - v2.y) * (v1.z + v2.z);
        normal.y += (v1.z - v2.z) * (v1.x + v2.x);
        normal.z += (v1.x - v2.x) * (v1.y + v2.y);
    }
    normal.normalize_or_zero()
}

fn side_planes(polygon: &[Vec3]) -> impl Iterator<Item = (usize, Plane)> + '_ {
    let normal = newell(polygon);
    (0..polygon.len()).map(move |index| {
        let a = polygon[index];
        let b = polygon[(index + 1) % polygon.len()];
        (
            index,
            Plane::from_normal_point((b - a).normalize_or_zero().cross(normal), a),
        )
    })
}

fn polygon_against_face(
    reference: &Face,
    incident: &Face,
    normal: Vec3,
    speculative: f32,
    out: &mut Candidates,
) {
    let ids = incident.ids();
    let reference = reference.points();
    let incident = incident.points();
    let mut reference_normal = newell(reference);
    if reference_normal.dot(normal) < 0.0 {
        reference_normal = -reference_normal;
    }
    let denominator = reference_normal.dot(normal);
    if denominator < FLAT {
        return;
    }
    let base = reference[0];
    let reference_plane = Plane::from_normal_point(reference_normal, base);

    let mut buffers = [[ClipVertex::default(); MAX_CLIP_POINTS]; 2];
    let mut count = incident.len().min(MAX_CLIP_POINTS);
    for (index, &position) in incident.iter().take(count).enumerate() {
        let next = (index + 1) % incident.len();
        buffers[0][index] = ClipVertex::new(
            position,
            FeaturePair::new(Owner::B, ids[index], Owner::B, ids[next]),
        );
    }

    let mut current = 0;
    if count == 2 {
        let mut segment = [buffers[0][0], buffers[0][1]];
        for (_, plane) in side_planes(reference) {
            if clip_segment(&mut segment, plane) < 2 {
                return;
            }
        }
        buffers[0][..2].copy_from_slice(&segment);
    } else {
        for (edge, plane) in side_planes(reference) {
            let [first, second] = &mut buffers;
            let (input, output) = if current == 0 {
                (first, second)
            } else {
                (second, first)
            };
            if let Some(clipped) =
                clip_polygon(output, &input[..count], plane, edge as u16, reference_plane)
            {
                count = clipped;
                current = 1 - current;
            }
            if count == 0 {
                return;
            }
        }
    }

    for vertex in &buffers[current][..count] {
        let separation = (vertex.point() - base).dot(reference_normal) / denominator;
        out.push(
            vertex.point() - 0.5 * separation * normal,
            separation,
            vertex.pair,
            speculative,
        );
    }
}

fn segment_against_face(
    segment: &Face,
    polygon: &Face,
    normal: Vec3,
    speculative: f32,
    out: &mut Candidates,
) {
    let ids = segment.ids();
    let segment = segment.points();
    let polygon = polygon.points();
    let mut polygon_normal = newell(polygon);
    if polygon_normal.dot(normal) > 0.0 {
        polygon_normal = -polygon_normal;
    }
    let denominator = -polygon_normal.dot(normal);
    if denominator < FLAT {
        return;
    }
    let mut clipped = [
        ClipVertex::new(
            segment[0],
            FeaturePair::new(Owner::A, ids[0], Owner::A, ids[0]),
        ),
        ClipVertex::new(
            segment[1],
            FeaturePair::new(Owner::A, ids[1], Owner::A, ids[1]),
        ),
    ];
    for (_, plane) in side_planes(polygon) {
        if clip_segment(&mut clipped, plane) < 2 {
            return;
        }
    }
    for vertex in clipped {
        let separation = (vertex.point() - polygon[0]).dot(polygon_normal) / denominator;
        out.push(
            vertex.point() + 0.5 * separation * normal,
            separation,
            vertex.pair,
            speculative,
        );
    }
}

fn segments(a: &Face, b: &Face, normal: Vec3, speculative: f32, out: &mut Candidates) {
    let ids = b.ids();
    let a = a.points();
    let b = b.points();
    let along_a = (a[1] - a[0]).normalize_or_zero();
    let along_b = (b[1] - b[0]).normalize_or_zero();
    if along_a.cross(along_b).length_squared() > PARALLEL * PARALLEL {
        return;
    }
    let mut clipped = [
        ClipVertex::new(b[0], FeaturePair::new(Owner::B, ids[0], Owner::B, ids[0])),
        ClipVertex::new(b[1], FeaturePair::new(Owner::B, ids[1], Owner::B, ids[1])),
    ];
    if clip_segment(&mut clipped, Plane::new(-along_a, -along_a.dot(a[0]))) < 2
        || clip_segment(&mut clipped, Plane::new(along_a, along_a.dot(a[1]))) < 2
    {
        return;
    }
    for vertex in clipped {
        let on_a = closest_on_segment(a[0], a[1], vertex.point());
        let separation = (vertex.point() - on_a).dot(normal);
        out.push(
            0.5 * (on_a + vertex.point()),
            separation,
            vertex.pair,
            speculative,
        );
    }
}

pub(crate) fn collide_general(
    a: Convex<'_>,
    b: Convex<'_>,
    transform: Transform,
    speculative: f32,
    cache: &mut SimplexCache,
) -> Manifold {
    let Some(features) = closest_features(&a, &b, transform, cache) else {
        return Manifold::default();
    };
    if features.separation > speculative {
        return Manifold::default();
    }
    let normal = features.normal;
    let face_a = supporting_face(a, normal);
    let face_b = supporting_face(b, transform.rotate_inverse(-normal)).transformed(transform);

    let mut candidates = Candidates {
        points: [ManifoldPoint::default(); MAX_CLIP_POINTS],
        count: 0,
    };
    match (face_a.len(), face_b.len()) {
        (3.., 2..) => polygon_against_face(&face_a, &face_b, normal, speculative, &mut candidates),
        (2, 3..) => segment_against_face(&face_a, &face_b, normal, speculative, &mut candidates),
        (2, 2) => segments(&face_a, &face_b, normal, speculative, &mut candidates),
        _ => {}
    }

    let mut manifold = Manifold {
        normal,
        ..Manifold::default()
    };
    if candidates.count > 0 {
        reduce(
            &mut manifold,
            &mut candidates.points[..candidates.count],
            speculative,
        );
    } else {
        manifold.push(point(
            features.point_a.lerp(features.point_b, 0.5),
            features.separation,
            FeaturePair::SINGLE,
        ));
    }
    manifold
}
