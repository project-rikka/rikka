use glam::{Vec2, Vec3};
use rikka_spatial::{
    FillRule, Plane, SegmentIntersection2d, barycentric_weights, intersect_segments_2d,
    mean_value_weights, mean_value_weights_2d, point_in_polygon, polygon_area, polygon_normal,
    signed_area_2d, signed_mesh_volume, signed_volume, winding_number,
};

use crate::support::Rng;

fn pentagram() -> Vec<Vec2> {
    (0..5)
        .map(|index| {
            let angle =
                std::f32::consts::FRAC_PI_2 + (index * 2) as f32 * std::f32::consts::TAU / 5.0;
            Vec2::new(angle.cos(), angle.sin())
        })
        .collect()
}

fn crossings_reference(point: Vec2, polygon: &[Vec2]) -> bool {
    let mut inside = false;
    for index in 0..polygon.len() {
        let (a, b) = (
            polygon[index].as_dvec2(),
            polygon[(index + 1) % polygon.len()].as_dvec2(),
        );
        let p = point.as_dvec2();
        if (a.y > p.y) != (b.y > p.y) {
            let x = a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y);
            if p.x < x {
                inside = !inside;
            }
        }
    }
    inside
}

#[test]
fn fill_rules_differ_inside_self_intersections() {
    let star = pentagram();
    assert_eq!(winding_number(Vec2::ZERO, &star), 2);
    assert!(!point_in_polygon(Vec2::ZERO, &star, FillRule::EvenOdd));
    assert!(point_in_polygon(Vec2::ZERO, &star, FillRule::NonZero));
    let tip = Vec2::new(0.0, 0.8);
    assert!(
        point_in_polygon(tip, &star, FillRule::EvenOdd)
            && point_in_polygon(tip, &star, FillRule::NonZero)
    );
    assert!(!point_in_polygon(
        Vec2::new(0.0, 1.2),
        &star,
        FillRule::NonZero
    ));
    let reversed: Vec<Vec2> = star.iter().rev().copied().collect();
    assert_eq!(winding_number(Vec2::ZERO, &reversed), -2);
}

#[test]
fn even_odd_matches_crossing_reference_on_random_polygons() {
    let mut rng = Rng::new(40);
    for _ in 0..500 {
        let polygon: Vec<Vec2> = (0..3 + rng.index(10))
            .map(|_| rng.vec2(-1.0, 1.0))
            .collect();
        for _ in 0..50 {
            let point = rng.vec2(-1.2, 1.2);
            assert_eq!(
                point_in_polygon(point, &polygon, FillRule::EvenOdd),
                crossings_reference(point, &polygon)
            );
        }
    }
}

#[test]
fn segment_intersection_2d_cases() {
    let cross = intersect_segments_2d(
        [Vec2::ZERO, Vec2::new(2.0, 2.0)],
        [Vec2::new(0.0, 2.0), Vec2::new(2.0, 0.0)],
    );
    let Some(SegmentIntersection2d::Point(crossing)) = cross else {
        panic!("expected crossing")
    };
    assert!(
        (crossing.point - Vec2::ONE).length() < 1e-6
            && crossing.first == 0.5
            && crossing.second == 0.5
    );
    let touching = intersect_segments_2d([Vec2::ZERO, Vec2::X], [Vec2::X, Vec2::new(1.0, 1.0)]);
    let Some(SegmentIntersection2d::Point(crossing)) = touching else {
        panic!("expected touch")
    };
    assert_eq!(
        (crossing.point, crossing.first, crossing.second),
        (Vec2::X, 1.0, 0.0)
    );
    let t_junction = intersect_segments_2d(
        [Vec2::ZERO, Vec2::new(4.0, 0.0)],
        [Vec2::new(1.0, 0.0), Vec2::new(1.0, 3.0)],
    );
    let Some(SegmentIntersection2d::Point(crossing)) = t_junction else {
        panic!("expected junction")
    };
    assert_eq!(
        (crossing.point, crossing.first, crossing.second),
        (Vec2::X, 0.25, 0.0)
    );
    assert!(intersect_segments_2d([Vec2::ZERO, Vec2::X], [Vec2::Y, Vec2::new(1.0, 1.0)]).is_none());
    assert!(
        intersect_segments_2d(
            [Vec2::ZERO, Vec2::X],
            [Vec2::new(2.0, 0.0), Vec2::new(3.0, 0.0)]
        )
        .is_none()
    );
    assert!(
        intersect_segments_2d(
            [Vec2::ZERO, Vec2::new(1.0, 1.0)],
            [Vec2::new(2.0, 0.0), Vec2::new(1.1, 0.95)]
        )
        .is_none()
    );
    let overlap = intersect_segments_2d(
        [Vec2::ZERO, Vec2::new(4.0, 0.0)],
        [Vec2::new(3.0, 0.0), Vec2::new(1.0, 0.0)],
    );
    let Some(SegmentIntersection2d::Overlap { start, end }) = overlap else {
        panic!("expected overlap")
    };
    assert_eq!(
        (start.point, start.first, start.second),
        (Vec2::X, 0.25, 1.0)
    );
    assert_eq!(
        (end.point, end.first, end.second),
        (Vec2::new(3.0, 0.0), 0.75, 0.0)
    );
    let collinear_touch =
        intersect_segments_2d([Vec2::ZERO, Vec2::X], [Vec2::X, Vec2::new(2.0, 0.0)]);
    assert!(
        matches!(collinear_touch, Some(SegmentIntersection2d::Point(crossing)) if crossing.point == Vec2::X)
    );
    let point_on = intersect_segments_2d([Vec2::new(0.5, 0.0); 2], [Vec2::ZERO, Vec2::X]);
    assert!(
        matches!(point_on, Some(SegmentIntersection2d::Point(crossing)) if crossing.second == 0.5)
    );
}

#[test]
fn segment_intersection_2d_matches_reference() {
    let mut rng = Rng::new(41);
    for _ in 0..20_000 {
        let first = [rng.vec2(-1.0, 1.0), rng.vec2(-1.0, 1.0)];
        let second = [rng.vec2(-1.0, 1.0), rng.vec2(-1.0, 1.0)];
        let (a, b) = (
            first[0].as_dvec2(),
            first[1].as_dvec2() - first[0].as_dvec2(),
        );
        let (c, d) = (
            second[0].as_dvec2(),
            second[1].as_dvec2() - second[0].as_dvec2(),
        );
        let denominator = b.perp_dot(d);
        let s = (c - a).perp_dot(d) / denominator;
        let t = (c - a).perp_dot(b) / denominator;
        let expected = (0.0..=1.0).contains(&s) && (0.0..=1.0).contains(&t);
        match intersect_segments_2d(first, second) {
            Some(SegmentIntersection2d::Point(crossing)) => {
                assert!(expected || s.min(t) > -1e-6 && s.max(t) < 1.0 + 1e-6);
                assert!(
                    (f64::from(crossing.first) - s).abs() < 1e-3
                        && (f64::from(crossing.second) - t).abs() < 1e-3
                );
            }
            Some(SegmentIntersection2d::Overlap { .. }) => panic!("random segments overlap"),
            None => assert!(!expected || s.min(t) < 1e-6 || s.max(t) > 1.0 - 1e-6),
        }
    }
}

#[test]
fn mean_value_weights_reproduce_points_in_convex_polygons() {
    let mut rng = Rng::new(42);
    for _ in 0..2_000 {
        let count = 3 + rng.index(6);
        let radius = rng.range(0.5, 2.0);
        let center = rng.vec3(-1.0, 1.0);
        let polygon: Vec<Vec3> = (0..count)
            .map(|index| {
                let angle =
                    index as f32 / count as f32 * std::f32::consts::TAU + rng.range(0.0, 0.5);
                center + Vec3::new(angle.cos(), angle.sin(), 0.0) * radius
            })
            .collect();
        let point = center + Vec3::new(rng.range(-0.3, 0.3), rng.range(-0.3, 0.3), 0.0) * radius;
        let mut weights = vec![0.0; count];
        mean_value_weights(&polygon, point, &mut weights);
        let rebuilt: Vec3 = polygon
            .iter()
            .zip(&weights)
            .map(|(vertex, weight)| *vertex * *weight)
            .sum();
        assert!(rebuilt.distance(point) < 1e-4, "{rebuilt} vs {point}");
        assert!((weights.iter().sum::<f32>() - 1.0).abs() < 1e-5);
        assert!(weights.iter().all(|weight| *weight > 0.0));
        let flat: Vec<Vec2> = polygon.iter().map(|vertex| vertex.truncate()).collect();
        let mut flat_weights = vec![0.0; count];
        mean_value_weights_2d(&flat, point.truncate(), &mut flat_weights);
        for (weight, flat_weight) in weights.iter().zip(&flat_weights) {
            assert!((weight - flat_weight).abs() < 1e-4);
        }
    }
}

#[test]
fn mean_value_weights_snap_to_vertices_and_edges() {
    let square = [Vec3::ZERO, Vec3::X, Vec3::new(1.0, 1.0, 0.0), Vec3::Y];
    let mut weights = [0.0; 4];
    mean_value_weights(&square, Vec3::X, &mut weights);
    assert_eq!(weights, [0.0, 1.0, 0.0, 0.0]);
    mean_value_weights(&square, Vec3::new(1.0, 0.25, 0.0), &mut weights);
    assert!(
        (weights[1] - 0.75).abs() < 1e-6 && (weights[2] - 0.25).abs() < 1e-6 && weights[0] == 0.0
    );
    mean_value_weights(&square, Vec3::new(0.0, 0.5, 0.0), &mut weights);
    assert!((weights[3] - 0.5).abs() < 1e-6 && (weights[0] - 0.5).abs() < 1e-6);
    let triangle = [Vec3::ZERO, Vec3::X, Vec3::Y];
    let mut triangle_weights = [0.0; 3];
    mean_value_weights(&triangle, Vec3::new(0.2, 0.3, 0.0), &mut triangle_weights);
    let expected = barycentric_weights(Vec3::new(0.2, 0.3, 0.0), triangle);
    assert!((Vec3::from_array(triangle_weights) - expected).length() < 1e-5);
    let mut flat = [0.0; 4];
    mean_value_weights_2d(
        &square.map(|vertex| vertex.truncate()),
        Vec2::new(0.5, 1.0),
        &mut flat,
    );
    assert!((flat[2] - 0.5).abs() < 1e-6 && (flat[3] - 0.5).abs() < 1e-6);
}

#[test]
fn polygon_measures() {
    let square = [
        Vec3::ZERO,
        Vec3::new(2.0, 0.0, 0.0),
        Vec3::new(2.0, 2.0, 0.0),
        Vec3::new(0.0, 2.0, 0.0),
    ];
    assert!((polygon_normal(&square) - Vec3::new(0.0, 0.0, 4.0)).length() < 1e-6);
    assert!((polygon_area(&square) - 4.0).abs() < 1e-6);
    let tilted = square
        .map(|vertex| Vec3::new(vertex.x, vertex.y * 0.6, vertex.y * 0.8) + Vec3::splat(10.0));
    let plane = Plane::from_polygon(&tilted).unwrap();
    assert!((plane.normal - Vec3::new(0.0, -0.8, 0.6)).length() < 1e-5);
    assert!(
        tilted
            .iter()
            .all(|vertex| plane.signed_distance(*vertex).abs() < 1e-5)
    );
    assert!(Plane::from_polygon(&[Vec3::ZERO, Vec3::X, Vec3::X * 2.0]).is_none());
    let flat = square.map(|vertex| vertex.truncate());
    assert_eq!(signed_area_2d(&flat), 4.0);
    let clockwise: Vec<Vec2> = flat.iter().rev().copied().collect();
    assert_eq!(signed_area_2d(&clockwise), -4.0);
    assert!((signed_volume(Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z) - 1.0 / 6.0).abs() < 1e-7);
    let corners =
        |x: u32, y: u32, z: u32| Vec3::new(x as f32, y as f32, z as f32) * 2.0 + Vec3::splat(5.0);
    let quads = [
        [(0, 0, 0), (0, 1, 0), (1, 1, 0), (1, 0, 0)],
        [(0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)],
        [(0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1)],
        [(0, 1, 0), (0, 1, 1), (1, 1, 1), (1, 1, 0)],
        [(0, 0, 0), (0, 0, 1), (0, 1, 1), (0, 1, 0)],
        [(1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1)],
    ];
    let triangles = quads.iter().flat_map(|quad| {
        let [a, b, c, d] = quad.map(|(x, y, z)| corners(x, y, z));
        [[a, b, c], [a, c, d]]
    });
    assert!((signed_mesh_volume(triangles) - 8.0).abs() < 1e-4);
}
