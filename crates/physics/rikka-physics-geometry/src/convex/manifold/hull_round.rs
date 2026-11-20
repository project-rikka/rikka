use rikka_physics_math::{Plane, Transform, Vec3};

use super::clip::{ClipVertex, FeaturePair, Owner, clip_segment};
use super::round::point;
use crate::convex::capsule::Capsule;
use crate::convex::distance::{
    DistanceInput, DistanceOutput, SimplexCache, line_distance, shape_distance,
};
use crate::convex::hull::Polyhedron;
use crate::convex::sphere::Sphere;
use crate::convex::support::point_support;
use crate::{Manifold, ShapeProxy, Tolerance};

const PARALLEL_EDGE_TOLERANCE: f32 = 0.005;
pub(crate) const SHALLOW: f32 = 100.0 * f32::EPSILON;

pub(crate) fn collide_polyhedron_sphere(
    hull: &Polyhedron<'_>,
    sphere: &Sphere,
    transform: Transform,
    speculative: f32,
    cache: &mut SimplexCache,
) -> Manifold {
    let center = transform.apply(sphere.center);
    let proxy = ShapeProxy::new(std::slice::from_ref(&sphere.center), 0.0);
    let output = shape_distance(&DistanceInput::new(hull, &proxy, transform), cache);
    let radius = sphere.radius;

    if output.distance > radius + speculative {
        cache.reset();
        return Manifold::default();
    }

    let (normal, separation) = if output.distance > SHALLOW {
        (
            (output.point_b - output.point_a).normalize(),
            output.distance - radius,
        )
    } else {
        let mut best = 0;
        let mut best_distance = -f32::MAX;
        for (index, plane) in hull.planes.iter().enumerate() {
            let distance = plane.separation(center);
            if distance > best_distance {
                best = index;
                best_distance = distance;
            }
        }
        (hull.planes[best].normal, best_distance - radius)
    };

    let on_hull = center - (center - output.point_a).dot(normal) * normal;
    let deepest = center - radius * normal;
    let mut manifold = Manifold {
        normal,
        ..Manifold::default()
    };
    manifold.push(point(
        on_hull.lerp(deepest, 0.5),
        separation,
        FeaturePair::SINGLE,
    ));
    manifold
}

pub(crate) trait CapsuleFeatures {
    fn planes(&self) -> impl Iterator<Item = Plane>;

    fn plane(&self, face: usize) -> Plane;

    fn edge_pair_count(&self) -> usize;

    fn edge_normals(&self, pair: usize) -> (Vec3, Vec3);

    fn edge_points(&self, edge: usize) -> (Vec3, Vec3);

    fn clip_to_face(&self, segment: &mut [ClipVertex; 2], face: usize) -> usize;
}

impl CapsuleFeatures for Polyhedron<'_> {
    #[inline]
    fn planes(&self) -> impl Iterator<Item = Plane> {
        self.planes.iter().copied()
    }

    #[inline]
    fn plane(&self, face: usize) -> Plane {
        self.planes[face]
    }

    #[inline]
    fn edge_pair_count(&self) -> usize {
        self.edges.len() / 2
    }

    #[inline]
    fn edge_normals(&self, pair: usize) -> (Vec3, Vec3) {
        (
            self.planes[self.edges[2 * pair].face as usize].normal,
            self.planes[self.edges[2 * pair + 1].face as usize].normal,
        )
    }

    #[inline]
    fn edge_points(&self, edge: usize) -> (Vec3, Vec3) {
        Polyhedron::edge_points(self, edge)
    }

    fn clip_to_face(&self, segment: &mut [ClipVertex; 2], face: usize) -> usize {
        let normal = self.planes[face].normal;
        for edge in self.face_edges(face) {
            let (vertex1, vertex2) = (
                self.points[self.edges[edge].origin as usize],
                self.points[self.edges[self.edges[edge].next as usize].origin as usize],
            );
            let binormal = (vertex2 - vertex1).cross(normal);
            if clip_segment(segment, Plane::from_normal_point(binormal, vertex1)) < 2 {
                return 0;
            }
        }
        2
    }
}

struct SeparatingAxis {
    normal: Vec3,
    separation: f32,
    index_a: usize,
    index_b: usize,
}

fn face_query(features: &impl CapsuleFeatures, points: &[Vec3; 2]) -> SeparatingAxis {
    let mut best = SeparatingAxis {
        normal: Vec3::ZERO,
        separation: -f32::MAX,
        index_a: 0,
        index_b: 0,
    };
    for (index, plane) in features.planes().enumerate() {
        let vertex = point_support(points, -plane.normal);
        let separation = plane.separation(points[vertex]);
        if separation > best.separation {
            best = SeparatingAxis {
                normal: plane.normal,
                separation,
                index_a: index,
                index_b: vertex,
            };
        }
    }
    best
}

fn edge_query(features: &impl CapsuleFeatures, points: &[Vec3; 2]) -> Option<SeparatingAxis> {
    let [pa, qa] = *points;
    let ea = qa - pa;
    let tolerance = PARALLEL_EDGE_TOLERANCE * PARALLEL_EDGE_TOLERANCE;
    let mut best: Option<SeparatingAxis> = None;
    for pair in 0..features.edge_pair_count() {
        let index = 2 * pair;
        let (_, qb) = features.edge_points(index);
        let (ub, vb) = features.edge_normals(pair);
        let cba = ub.dot(ea);
        let dba = vb.dot(ea);
        if cba * dba >= 0.0 || (cba * cba).max(dba * dba) < tolerance * ea.length_squared() {
            continue;
        }
        let t = cba / (cba - dba);
        let axis = ub.lerp(vb, t).normalize();
        let separation = axis.dot(qa - qb);
        if best
            .as_ref()
            .is_none_or(|current| separation > current.separation)
        {
            best = Some(SeparatingAxis {
                normal: axis,
                separation,
                index_a: 0,
                index_b: index,
            });
        }
    }
    best
}

pub(crate) fn two_point(
    normal: Vec3,
    segment: &[ClipVertex; 2],
    distances: [f32; 2],
    radius: f32,
) -> Manifold {
    let mut manifold = Manifold {
        normal,
        ..Manifold::default()
    };
    for (vertex, distance) in segment.iter().zip(distances) {
        let contact = vertex.point() - 0.5 * (radius + distance) * normal;
        manifold.push(point(contact, distance - radius, vertex.pair));
    }
    manifold
}

pub(crate) fn capsule_segment(points: [Vec3; 2]) -> [ClipVertex; 2] {
    [
        ClipVertex::new(points[0], FeaturePair::new(Owner::A, 0, Owner::A, 0)),
        ClipVertex::new(points[1], FeaturePair::new(Owner::A, 1, Owner::A, 1)),
    ]
}

pub(crate) fn collide_polyhedron_capsule(
    hull: &Polyhedron<'_>,
    capsule: &Capsule,
    transform: Transform,
    speculative: f32,
    tolerance: Tolerance,
    cache: &mut SimplexCache,
) -> Manifold {
    let local = [capsule.center1, capsule.center2];
    let proxy = ShapeProxy::new(&local, 0.0);
    let output = shape_distance(&DistanceInput::new(hull, &proxy, transform), cache);
    let radius = capsule.radius1;
    let points = [
        transform.apply(capsule.center1),
        transform.apply(capsule.center2),
    ];

    if output.distance > radius + speculative {
        cache.reset();
        return Manifold::default();
    }

    if output.distance > SHALLOW {
        let delta = output.normal;
        let face = hull.support_face(delta);
        let plane = hull.planes[face];
        if plane.normal.dot(delta).abs() > 0.998 {
            let mut segment = capsule_segment(points);
            if hull.clip_to_face(&mut segment, face) == 2 {
                let distances = segment.map(|vertex| plane.separation(vertex.point()));
                if distances[0] <= radius + speculative || distances[1] <= radius + speculative {
                    return two_point(plane.normal, &segment, distances, radius);
                }
            }
        }
        return closest_contact(&output, radius);
    }
    deep_capsule(hull, points, radius, speculative, tolerance)
}

pub(crate) fn closest_contact(output: &DistanceOutput, radius: f32) -> Manifold {
    let normal = output.normal;
    let mut manifold = Manifold {
        normal,
        ..Manifold::default()
    };
    manifold.push(point(
        0.5 * (output.point_a - radius * normal + output.point_b),
        output.distance - radius,
        FeaturePair::SINGLE,
    ));
    manifold
}

pub(crate) fn deep_capsule(
    hull: &impl CapsuleFeatures,
    points: [Vec3; 2],
    radius: f32,
    speculative: f32,
    tolerance: Tolerance,
) -> Manifold {
    let face = face_query(hull, &points);
    if face.separation > radius {
        return Manifold::default();
    }
    let edge = edge_query(hull, &points);
    if edge.as_ref().is_some_and(|edge| edge.separation > radius) {
        return Manifold::default();
    }

    let mut manifold = Manifold::default();
    let mut face_separation = face.separation - radius;
    let plane = hull.plane(face.index_a);
    let mut segment = capsule_segment(points);
    if hull.clip_to_face(&mut segment, face.index_a) == 2 {
        let distances = segment.map(|vertex| plane.separation(vertex.point()));
        if distances[0] <= speculative || distances[1] <= speculative {
            manifold = two_point(plane.normal, &segment, distances, radius);
            face_separation = manifold.points[0]
                .separation
                .min(manifold.points[1].separation);
        }
    }

    let Some(edge) = edge else {
        return manifold;
    };
    let edge_separation = edge.separation - radius;
    if manifold.is_empty() || edge_separation > face_separation + tolerance.linear_slop() {
        let (ph, qh) = hull.edge_points(edge.index_b);
        let result = line_distance(ph, qh - ph, points[0], points[1] - points[0]);
        if result.is_within_segments() {
            let normal = edge.normal;
            let contact = 0.5 * (result.point1 - radius * normal + result.point2);
            let separation = normal.dot(result.point2 - result.point1);
            let mut edge_manifold = Manifold {
                normal,
                ..Manifold::default()
            };
            edge_manifold.push(point(
                contact,
                separation - radius,
                FeaturePair::new(Owner::A, edge.index_a as u16, Owner::B, edge.index_b as u16),
            ));
            return edge_manifold;
        }
    }
    manifold
}
