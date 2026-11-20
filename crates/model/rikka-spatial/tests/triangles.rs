use glam::Vec3;
use rikka_spatial::{
    Aabb, Containment, Frustum, Plane, TriangleIntersection, barycentric_weights,
    intersect_triangles, triangle_area, triangle_intersects_aabb, triangle_normal,
};

use crate::support::{Rng, triangle_distance_reference};

fn edge_crossings(first: [Vec3; 3], second: [Vec3; 3]) -> Vec<Vec3> {
    let mut points = Vec::new();
    for (edges, target) in [(first, second), (second, first)] {
        for index in 0..3 {
            let (start, end) = (edges[index], edges[(index + 1) % 3]);
            let direction = end - start;
            if let Some(t) = crate::support::ray_triangle_reference(start, direction, target)
                && t <= 1.0
            {
                points.push(start + direction * t as f32);
            }
        }
    }
    points
}

fn segment_of(intersection: &TriangleIntersection) -> (Vec3, Vec3) {
    match intersection {
        TriangleIntersection::Segment { start, end } => (*start, *end),
        TriangleIntersection::Coplanar { .. } => panic!("unexpected coplanar result"),
    }
}

#[test]
fn triangle_intersection_matches_edge_crossing_reference() {
    let mut rng = Rng::new(30);
    let mut intersecting = 0;
    for case in 0..20_000 {
        let first = [
            rng.vec3(-1.0, 1.0),
            rng.vec3(-1.0, 1.0),
            rng.vec3(-1.0, 1.0),
        ];
        let second = [
            rng.vec3(-1.0, 1.0),
            rng.vec3(-1.0, 1.0),
            rng.vec3(-1.0, 1.0),
        ];
        let crossings = edge_crossings(first, second);
        let result = intersect_triangles(first, second);
        if crossings.is_empty() {
            if let Some(intersection) = &result {
                let (start, end) = segment_of(intersection);
                assert!(
                    start.distance(end) < 1e-3,
                    "case {case}: missed reference segment"
                );
            }
            continue;
        }
        intersecting += 1;
        let (start, end) = segment_of(
            result
                .as_ref()
                .unwrap_or_else(|| panic!("case {case}: no intersection")),
        );
        let mut longest = 0.0f32;
        for a in &crossings {
            for b in &crossings {
                longest = longest.max(a.distance(*b));
            }
        }
        assert!(
            (start.distance(end) - longest).abs() < 1e-3,
            "case {case}: length {} vs {longest}",
            start.distance(end)
        );
        for point in [start, end] {
            assert!(
                triangle_distance_reference(point, first) < 1e-4,
                "case {case}"
            );
            assert!(
                triangle_distance_reference(point, second) < 1e-4,
                "case {case}"
            );
        }
    }
    assert!(intersecting > 1_000);
}

#[test]
fn triangle_intersection_handles_touching_configurations() {
    let base = [Vec3::ZERO, Vec3::X, Vec3::Y];
    let cases = [
        ([Vec3::ZERO, Vec3::X, Vec3::Z], Some((Vec3::ZERO, Vec3::X))),
        (
            [
                Vec3::ZERO,
                Vec3::new(-1.0, 0.0, 1.0),
                Vec3::new(0.0, -1.0, 1.0),
            ],
            Some((Vec3::ZERO, Vec3::ZERO)),
        ),
        (
            [
                Vec3::new(0.25, 0.25, 0.0),
                Vec3::new(1.0, 1.0, 1.0),
                Vec3::new(0.0, 1.0, 1.0),
            ],
            Some((Vec3::new(0.25, 0.25, 0.0), Vec3::new(0.25, 0.25, 0.0))),
        ),
        (
            [
                Vec3::new(-1.0, 0.25, 0.0),
                Vec3::new(2.0, 0.25, 0.0),
                Vec3::new(0.5, 0.25, 1.0),
            ],
            Some((Vec3::new(0.0, 0.25, 0.0), Vec3::new(0.75, 0.25, 0.0))),
        ),
        (
            [
                Vec3::new(0.2, 0.2, -1.0),
                Vec3::new(0.2, 0.2, 1.0),
                Vec3::new(5.0, 5.0, 0.0),
            ],
            Some((Vec3::new(0.2, 0.2, 0.0), Vec3::new(0.5, 0.5, 0.0))),
        ),
        (
            [
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(0.0, 1.0, 1.0),
            ],
            None,
        ),
        (
            [
                Vec3::new(0.6, 0.6, -1.0),
                Vec3::new(0.6, 0.6, 1.0),
                Vec3::new(3.0, 3.0, 0.0),
            ],
            None,
        ),
    ];
    for (index, (other, expected)) in cases.into_iter().enumerate() {
        let result = intersect_triangles(base, other);
        let reversed = intersect_triangles(other, base);
        match expected {
            None => assert!(
                result.is_none() && reversed.is_none(),
                "case {index}: {result:?}"
            ),
            Some((start, end)) => {
                for result in [result, reversed] {
                    let (actual_start, actual_end) =
                        segment_of(&result.unwrap_or_else(|| panic!("case {index} missed")));
                    let same =
                        actual_start.distance(start) < 1e-6 && actual_end.distance(end) < 1e-6;
                    let swapped =
                        actual_start.distance(end) < 1e-6 && actual_end.distance(start) < 1e-6;
                    assert!(same || swapped, "case {index}: {actual_start} {actual_end}");
                }
            }
        }
    }
}

#[test]
fn coplanar_triangles_intersect_in_polygons() {
    let base = [
        Vec3::ZERO,
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(0.0, 2.0, 0.0),
    ];
    let overlapping = [
        Vec3::new(0.5, 0.5, 0.0),
        Vec3::new(3.0, 0.5, 0.0),
        Vec3::new(0.5, 3.0, 0.0),
    ];
    let Some(TriangleIntersection::Coplanar { polygon }) = intersect_triangles(base, overlapping)
    else {
        panic!("expected coplanar overlap");
    };
    let area: f32 = (1..polygon.len() - 1)
        .map(|index| triangle_area([polygon[0], polygon[index], polygon[index + 1]]))
        .sum();
    assert!((area - 0.5).abs() < 1e-5, "area {area}");
    let touching = [
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(3.0, 0.0, 0.0),
        Vec3::new(3.0, 1.0, 0.0),
    ];
    let Some(TriangleIntersection::Coplanar { polygon }) = intersect_triangles(base, touching)
    else {
        panic!("expected coplanar contact");
    };
    assert_eq!(polygon, vec![Vec3::new(2.0, 0.0, 0.0)]);
    let apart = [
        Vec3::new(2.5, 0.0, 0.0),
        Vec3::new(3.0, 0.0, 0.0),
        Vec3::new(3.0, 1.0, 0.0),
    ];
    assert!(intersect_triangles(base, apart).is_none());
    let inner = [
        Vec3::new(0.2, 0.2, 0.0),
        Vec3::new(0.6, 0.2, 0.0),
        Vec3::new(0.2, 0.6, 0.0),
    ];
    let Some(TriangleIntersection::Coplanar { polygon }) = intersect_triangles(base, inner) else {
        panic!("expected contained triangle");
    };
    assert_eq!(polygon.len(), 3);
}

#[test]
fn degenerate_triangles_intersect_as_segments() {
    let base = [Vec3::ZERO, Vec3::X, Vec3::Y];
    let needle = [
        Vec3::new(0.25, 0.25, -1.0),
        Vec3::new(0.25, 0.25, 1.0),
        Vec3::new(0.25, 0.25, 0.5),
    ];
    let (start, end) = segment_of(&intersect_triangles(base, needle).unwrap());
    assert!(start.distance(Vec3::new(0.25, 0.25, 0.0)) < 1e-6 && start == end);
    let lying = [
        Vec3::new(-1.0, 0.25, 0.0),
        Vec3::new(2.0, 0.25, 0.0),
        Vec3::new(0.5, 0.25, 0.0),
    ];
    let (start, end) = segment_of(&intersect_triangles(lying, base).unwrap());
    let ends = [start, end];
    assert!(
        ends.iter()
            .any(|point| point.distance(Vec3::new(0.0, 0.25, 0.0)) < 1e-6)
    );
    assert!(
        ends.iter()
            .any(|point| point.distance(Vec3::new(0.75, 0.25, 0.0)) < 1e-6)
    );
    let missing = [
        Vec3::new(0.75, 0.75, -1.0),
        Vec3::new(0.75, 0.75, 1.0),
        Vec3::new(0.75, 0.75, 0.0),
    ];
    assert!(intersect_triangles(base, missing).is_none());
    let cross = [
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 0.0),
    ];
    let other = [
        Vec3::new(0.5, -1.0, 0.0),
        Vec3::new(0.5, 1.0, 0.0),
        Vec3::new(0.5, 0.0, 0.0),
    ];
    let (start, end) = segment_of(&intersect_triangles(cross, other).unwrap());
    assert!(start.distance(Vec3::new(0.5, 0.0, 0.0)) < 1e-6 && start == end);
    let skew = [
        Vec3::new(0.5, -1.0, 0.1),
        Vec3::new(0.5, 1.0, 0.1),
        Vec3::new(0.5, 0.0, 0.1),
    ];
    assert!(intersect_triangles(cross, skew).is_none());
}

#[test]
fn triangle_aabb_test_matches_clipping_oracle() {
    let mut rng = Rng::new(31);
    let bounds = Aabb::new(Vec3::splat(-0.5), Vec3::splat(0.5));
    let box_planes = Frustum::new([
        Plane::new(Vec3::X, 0.5),
        Plane::new(Vec3::NEG_X, 0.5),
        Plane::new(Vec3::Y, 0.5),
        Plane::new(Vec3::NEG_Y, 0.5),
        Plane::new(Vec3::Z, 0.5),
        Plane::new(Vec3::NEG_Z, 0.5),
    ]);
    let mut mismatches = 0;
    for _ in 0..20_000 {
        let center = rng.vec3(-1.5, 1.5);
        let triangle = [
            center + rng.vec3(-1.0, 1.0),
            center + rng.vec3(-1.0, 1.0),
            center + rng.vec3(-1.0, 1.0),
        ];
        let expected = box_planes.classify_triangle(triangle) != Containment::Outside;
        if triangle_intersects_aabb(triangle, &bounds) != expected {
            mismatches += 1;
        }
    }
    assert!(mismatches <= 2, "{mismatches} mismatches");
    assert!(triangle_intersects_aabb(
        [
            Vec3::new(-2.0, 0.0, -2.0),
            Vec3::new(2.0, 0.0, -2.0),
            Vec3::new(0.0, 0.0, 2.0)
        ],
        &bounds
    ));
    assert!(!triangle_intersects_aabb(
        [
            Vec3::new(0.6, 0.6, 0.0),
            Vec3::new(2.0, 0.6, 0.0),
            Vec3::new(0.6, 2.0, 0.0)
        ],
        &bounds
    ));
    assert!(!triangle_intersects_aabb(
        [
            Vec3::new(1.2, 0.0, 0.0),
            Vec3::new(0.0, 1.2, 0.0),
            Vec3::new(1.2, 1.2, 0.0)
        ],
        &bounds
    ));
}

#[test]
fn barycentric_weights_reproduce_points() {
    let mut rng = Rng::new(32);
    for _ in 0..10_000 {
        let triangle = [
            rng.vec3(-1.0, 1.0),
            rng.vec3(-1.0, 1.0),
            rng.vec3(-1.0, 1.0),
        ];
        if triangle_area(triangle) < 1e-3 {
            continue;
        }
        let weights = Vec3::new(rng.range(-0.5, 1.5), rng.range(-0.5, 1.5), 0.0);
        let weights = Vec3::new(weights.x, weights.y, 1.0 - weights.x - weights.y);
        let point = triangle[0] * weights.x + triangle[1] * weights.y + triangle[2] * weights.z;
        let offset = triangle_normal(triangle) * rng.range(-1.0, 1.0);
        assert!((barycentric_weights(point + offset, triangle) - weights).length() < 1e-3);
    }
    let sliver = [Vec3::ZERO, Vec3::X, Vec3::X * 2.0];
    let weights = barycentric_weights(Vec3::new(1.5, 1.0, 0.0), sliver);
    assert!(
        (weights - Vec3::new(0.25, 0.0, 0.75)).length() < 1e-6,
        "{weights}"
    );
    assert_eq!(barycentric_weights(Vec3::ONE, [Vec3::Y; 3]), Vec3::X);
}
