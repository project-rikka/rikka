use glam::Vec3;
use rikka_spatial::{
    Ray, closest_point_on_segment, closest_point_on_triangle, closest_points_ray_segment,
    closest_points_segments,
};

use crate::support::{Rng, segment_distance_reference, triangle_distance_reference};

#[test]
fn closest_point_on_triangle_matches_reference() {
    let mut rng = Rng::new(20);
    for case in 0..50_000 {
        let mut triangle = [
            rng.vec3(-1.0, 1.0),
            rng.vec3(-1.0, 1.0),
            rng.vec3(-1.0, 1.0),
        ];
        match case % 10 {
            0 => triangle[2] = triangle[0].lerp(triangle[1], rng.range(-0.5, 1.5)),
            1 => triangle[1] = triangle[0],
            2 => triangle = [triangle[0]; 3],
            _ => {}
        }
        let point = rng.vec3(-2.0, 2.0);
        let closest = closest_point_on_triangle(point, triangle);
        let expected = triangle_distance_reference(point, triangle);
        let actual = f64::from(closest.point.distance(point));
        assert!(
            (actual - expected).abs() < 1e-5,
            "case {case}: {actual} vs {expected}"
        );
        let [a, b, c] = triangle;
        let rebuilt =
            a * closest.barycentric.x + b * closest.barycentric.y + c * closest.barycentric.z;
        assert!(rebuilt.distance(closest.point) < 1e-5, "case {case}");
        assert!(
            closest.barycentric.min_element() >= -1e-6
                && (closest.barycentric.element_sum() - 1.0).abs() < 1e-5
        );
    }
}

#[test]
fn closest_point_on_triangle_regions() {
    let triangle = [Vec3::ZERO, Vec3::X, Vec3::Y];
    let cases = [
        (Vec3::new(0.25, 0.25, 3.0), Vec3::new(0.25, 0.25, 0.0)),
        (Vec3::new(-1.0, -1.0, 0.0), Vec3::ZERO),
        (Vec3::new(2.0, -0.5, 1.0), Vec3::X),
        (Vec3::new(0.5, -2.0, 0.0), Vec3::new(0.5, 0.0, 0.0)),
        (Vec3::new(1.0, 1.0, 0.0), Vec3::new(0.5, 0.5, 0.0)),
        (Vec3::new(-0.5, 0.5, -1.0), Vec3::new(0.0, 0.5, 0.0)),
    ];
    for (point, expected) in cases {
        assert!(
            closest_point_on_triangle(point, triangle)
                .point
                .distance(expected)
                < 1e-6,
            "{point}"
        );
    }
}

#[test]
fn closest_point_on_segment_clamps_and_handles_points() {
    let closest = closest_point_on_segment(Vec3::new(0.5, 1.0, 0.0), Vec3::ZERO, Vec3::X);
    assert_eq!(
        (closest.point, closest.parameter),
        (Vec3::new(0.5, 0.0, 0.0), 0.5)
    );
    assert_eq!(
        closest_point_on_segment(Vec3::new(-1.0, 1.0, 0.0), Vec3::ZERO, Vec3::X).parameter,
        0.0
    );
    assert_eq!(
        closest_point_on_segment(Vec3::new(3.0, 1.0, 0.0), Vec3::ZERO, Vec3::X).parameter,
        1.0
    );
    let degenerate = closest_point_on_segment(Vec3::ONE, Vec3::Y, Vec3::Y);
    assert_eq!((degenerate.point, degenerate.parameter), (Vec3::Y, 0.0));
}

fn brute_segment_pair(first: [Vec3; 2], second: [Vec3; 2], first_limit: f64) -> f64 {
    let (a, b) = (first[0].as_dvec3(), first[1].as_dvec3());
    let mut best = f64::INFINITY;
    let steps = 400;
    for step in 0..=steps {
        let s = first_limit * step as f64 / steps as f64;
        let point = a + (b - a) * s;
        best = best.min(segment_distance_reference(
            point,
            second[0].as_dvec3(),
            second[1].as_dvec3(),
        ));
    }
    best
}

#[test]
fn closest_points_between_segments_match_brute_force() {
    let mut rng = Rng::new(21);
    for case in 0..3_000 {
        let mut first = [rng.vec3(-1.0, 1.0), rng.vec3(-1.0, 1.0)];
        let mut second = [rng.vec3(-1.0, 1.0), rng.vec3(-1.0, 1.0)];
        match case % 6 {
            0 => {
                second = [
                    first[0] + Vec3::Z * 0.3,
                    first[1] + Vec3::Z * 0.3 + (first[1] - first[0]) * 0.5,
                ]
            }
            1 => first[1] = first[0],
            2 => {
                first[1] = first[0];
                second[1] = second[0];
            }
            _ => {}
        }
        let pair = closest_points_segments(first, second);
        let expected = brute_segment_pair(first, second, 1.0);
        let actual = f64::from(pair.distance_squared.sqrt());
        assert!(
            actual <= expected + 1e-5,
            "case {case}: {actual} > {expected}"
        );
        assert!(
            actual >= expected - 2e-3,
            "case {case}: {actual} < {expected}"
        );
        assert!(
            (pair.first.point.distance(pair.second.point) - pair.distance_squared.sqrt()).abs()
                < 1e-5
        );
        assert!(
            first[0]
                .lerp(first[1], pair.first.parameter)
                .distance(pair.first.point)
                < 1e-5
        );
    }
}

#[test]
fn closest_points_ray_segment_match_brute_force() {
    let mut rng = Rng::new(22);
    for case in 0..3_000 {
        let origin = rng.vec3(-1.0, 1.0);
        let ray = Ray::new(origin, rng.direction()).unwrap();
        let segment = [rng.vec3(-1.0, 1.0), rng.vec3(-1.0, 1.0)];
        let limit = if case % 2 == 0 { 3.0 } else { 0.5 };
        let pair = closest_points_ray_segment(&ray, limit, segment);
        let expected = brute_segment_pair([origin, ray.at(1.0)], segment, f64::from(limit));
        let actual = f64::from(pair.distance_squared.sqrt());
        assert!(
            actual <= expected + 1e-5 && actual >= expected - 5e-3,
            "case {case}: {actual} vs {expected}"
        );
        assert!(pair.ray_distance >= 0.0 && pair.ray_distance <= limit);
        assert!(ray.at(pair.ray_distance).distance(pair.ray_point) < 1e-5);
    }
    let ray = Ray::new(Vec3::new(0.0, 0.0, 5.0), Vec3::X).unwrap();
    let parallel = closest_points_ray_segment(
        &ray,
        f32::INFINITY,
        [Vec3::new(2.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0)],
    );
    assert!((parallel.distance_squared - 25.0).abs() < 1e-4);
    let behind = closest_points_ray_segment(
        &ray,
        f32::INFINITY,
        [Vec3::new(-4.0, 0.0, 5.0), Vec3::new(-2.0, 0.0, 5.0)],
    );
    assert!((behind.distance_squared - 4.0).abs() < 1e-4 && behind.ray_distance == 0.0);
}
