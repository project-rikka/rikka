use rikka_physics_math::{Quat, Transform, Vec3};

use crate::support::{Random, close, face_rings};
use rikka_physics_geometry::{
    Capsule, ConvexHull, Cylinder, DistanceInput, SimplexCache, segment_distance, shape_distance,
};

pub(super) fn random_hull(random: &mut Random, count: usize, extent: f32) -> ConvexHull {
    loop {
        let points: Vec<Vec3> = (0..count).map(|_| random.vector(extent)).collect();
        if let Ok(hull) = ConvexHull::new(&points) {
            return hull;
        }
    }
}

fn triangles(hull: &ConvexHull, transform: Transform) -> Vec<[Vec3; 3]> {
    let mut out = Vec::new();
    for face in face_rings(hull) {
        let ring: Vec<Vec3> = face
            .into_iter()
            .map(|edge| transform.apply(hull.points()[hull.edges()[edge].origin as usize]))
            .collect();
        for index in 1..ring.len() - 1 {
            out.push([ring[0], ring[index], ring[index + 1]]);
        }
    }
    out
}

fn edges(hull: &ConvexHull, transform: Transform) -> Vec<(Vec3, Vec3)> {
    let points = hull.points();
    let edges = hull.edges();
    (0..edges.len())
        .step_by(2)
        .map(|edge| {
            let p = points[edges[edge].origin as usize];
            let q = points[edges[edges[edge].twin as usize].origin as usize];
            (transform.apply(p), transform.apply(q))
        })
        .collect()
}

fn closest_on_triangle(a: Vec3, b: Vec3, c: Vec3, p: Vec3) -> Vec3 {
    let normal = (b - a).cross(c - a).normalize();
    let projected = p - (p - a).dot(normal) * normal;
    let inside = [(a, b), (b, c), (c, a)]
        .iter()
        .all(|&(from, to)| (to - from).cross(projected - from).dot(normal) >= 0.0);
    if inside {
        return projected;
    }
    [(a, b), (b, c), (c, a)]
        .iter()
        .map(|&(from, to)| segment_distance(from, to, p, p).point1)
        .min_by(|x, y| x.distance(p).total_cmp(&y.distance(p)))
        .unwrap_or(a)
}

fn brute_distance(a: &ConvexHull, b: &ConvexHull, transform: Transform) -> f32 {
    let mut best = f32::MAX;
    for triangle in triangles(b, transform) {
        for &point in a.points() {
            best = best.min(
                closest_on_triangle(triangle[0], triangle[1], triangle[2], point).distance(point),
            );
        }
    }
    for triangle in triangles(a, Transform::IDENTITY) {
        for &point in b.points() {
            let point = transform.apply(point);
            best = best.min(
                closest_on_triangle(triangle[0], triangle[1], triangle[2], point).distance(point),
            );
        }
    }
    for (p1, q1) in edges(a, Transform::IDENTITY) {
        for &(p2, q2) in &edges(b, transform) {
            best = best.min(segment_distance(p1, q1, p2, q2).distance_squared().sqrt());
        }
    }
    best
}

fn brute_sat(a: &ConvexHull, b: &ConvexHull, transform: Transform) -> f32 {
    let points_b: Vec<Vec3> = b
        .points()
        .iter()
        .map(|&point| transform.apply(point))
        .collect();
    let separation = |axis: Vec3| {
        let max_a = a
            .points()
            .iter()
            .map(|point| axis.dot(*point))
            .fold(f32::MIN, f32::max);
        let min_b = points_b
            .iter()
            .map(|point| axis.dot(*point))
            .fold(f32::MAX, f32::min);
        min_b - max_a
    };
    let mut axes: Vec<Vec3> = a.planes().iter().map(|plane| plane.normal).collect();
    axes.extend(
        b.planes()
            .iter()
            .map(|plane| -transform.rotate(plane.normal)),
    );
    for (p1, q1) in edges(a, Transform::IDENTITY) {
        for &(p2, q2) in &edges(b, transform) {
            let cross = (q1 - p1).normalize().cross((q2 - p2).normalize());
            if cross.length() > 0.01 {
                axes.push(cross.normalize());
                axes.push(-cross.normalize());
            }
        }
    }
    axes.into_iter().map(separation).fold(f32::MIN, f32::max)
}

#[test]
fn gjk_matches_brute_force() {
    let mut random = Random::new(7);
    let mut checked = 0;
    for _ in 0..150 {
        let a = random_hull(&mut random, 12, 1.0);
        let b = random_hull(&mut random, 12, 1.0);
        let transform = random.transform(2.5);
        if brute_sat(&a, &b, transform) <= 1.0e-3 {
            continue;
        }
        let expected = brute_distance(&a, &b, transform);
        let mut cache = SimplexCache::EMPTY;
        let output = shape_distance(&DistanceInput::new(&a, &b, transform), &mut cache);
        assert!(
            close(output.distance, expected, 1.0e-4),
            "{} != {expected}",
            output.distance
        );
        assert!(close(
            output.point_a.distance(output.point_b),
            expected,
            1.0e-4
        ));
        let again = shape_distance(&DistanceInput::new(&a, &b, transform), &mut cache);
        assert!(close(again.distance, expected, 1.0e-4));
        checked += 1;
    }
    assert!(checked > 50);
}

#[test]
fn warm_curved_distances_match_cold() {
    let cylinder = Cylinder::new(0.6, 0.4);
    let capsule = Capsule::tapered(Vec3::ZERO, 0.5, Vec3::new(0.0, 0.9, 0.0), 0.2);
    let mut random = Random::new(41);
    let mut checked = 0;
    for _ in 0..40 {
        let start = random.transform(0.5);
        let offset = random.vector(1.0).normalize_or(Vec3::X) * 2.2;
        let mut cache = SimplexCache::EMPTY;
        for step in 0..30 {
            let angle = step as f32 * 0.03;
            let transform = Transform::new(
                start.p + offset * (1.0 - 0.01 * step as f32),
                start.q * Quat::from_rotation_z(angle),
            );
            let input = DistanceInput::new(&cylinder, &capsule, transform);
            let warm = shape_distance(&input, &mut cache);
            let cold = shape_distance(&input, &mut SimplexCache::default());
            if cold.distance < 1.0e-2 {
                cache.reset();
                continue;
            }
            assert!(
                close(warm.distance, cold.distance, 1.0e-4),
                "{} != {}",
                warm.distance,
                cold.distance
            );
            checked += 1;
        }
    }
    assert!(checked > 600);
}
