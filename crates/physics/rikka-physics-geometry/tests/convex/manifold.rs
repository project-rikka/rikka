use rikka_physics_math::{Quat, Transform, Vec3};

use crate::support::{Random, close, close_vec};
use rikka_physics_geometry::{
    Capsule, ConvexHull, Cuboid, Cylinder, DistanceInput, Manifold, PairCache, ProxyBuffer, Shape,
    SimplexCache, Sphere, collide, shape_distance,
};

const SPECULATIVE: f32 = 0.02;

fn run(a: &Shape, b: &Shape, transform: Transform) -> Manifold {
    collide(a, b, transform, SPECULATIVE, &mut PairCache::default())
}

fn up(offset: f32) -> Transform {
    Transform::from_translation(Vec3::new(0.0, offset, 0.0))
}

fn assert_resting(manifold: &Manifold, count: u32, separation: f32) {
    assert_eq!(manifold.count, count, "{manifold:?}");
    assert!(close_vec(manifold.normal, Vec3::Y, 1.0e-4), "{manifold:?}");
    for point in manifold.points() {
        assert!(close(point.separation, separation, 1.0e-3), "{manifold:?}");
    }
}

#[test]
fn box_rests_on_box_with_four_points() {
    let ground: Shape = Cuboid::new(Vec3::new(2.0, 0.5, 2.0)).into();
    let block: Shape = Cuboid::new(Vec3::splat(0.5)).into();
    for gap in [-0.004, 0.0, 0.01] {
        let manifold = run(&ground, &block, up(1.0 + gap));
        assert_resting(&manifold, 4, gap);
        for point in manifold.points() {
            assert!(close(point.point.y, 0.5 + 0.5 * gap, 1.0e-4));
        }
    }
    assert!(run(&ground, &block, up(1.0 + 2.0 * SPECULATIVE)).is_empty());
}

#[test]
fn pairs_beyond_speculative_margin_have_no_contacts() {
    let hull = ConvexHull::rock(0.4).expect("rock");
    let shapes: Vec<Shape> = vec![
        Sphere::new(0.3).into(),
        Capsule::new(0.2, 0.3).into(),
        Capsule::tapered(Vec3::ZERO, 0.3, Vec3::new(0.0, 0.5, 0.0), 0.1).into(),
        Cuboid::new(Vec3::new(0.3, 0.2, 0.4)).into(),
        Cylinder::new(0.3, 0.25).into(),
        hull.into(),
    ];
    let mut random = Random::new(41);
    for a in &shapes {
        for b in &shapes {
            for _ in 0..40 {
                let transform = random.transform(0.9);
                let manifold = run(a, b, transform);
                let mut buffer_a = ProxyBuffer::default();
                let mut buffer_b = ProxyBuffer::default();
                let proxy_a = a.proxy(&mut buffer_a).expect("convex");
                let proxy_b = b.proxy(&mut buffer_b).expect("convex");
                let distance = shape_distance(
                    &DistanceInput::new(&proxy_a, &proxy_b, transform).with_radii(),
                    &mut SimplexCache::default(),
                )
                .distance;
                if distance > SPECULATIVE + 1.0e-3 {
                    assert!(
                        manifold.is_empty(),
                        "{:?} {:?} {manifold:?}",
                        a.kind(),
                        b.kind()
                    );
                }
                assert!(manifold.count <= 4);
            }
        }
    }
}

#[test]
fn repeated_calls_are_deterministic() {
    let mut random = Random::new(51);
    let shapes: Vec<Shape> = vec![
        Cuboid::new(Vec3::splat(0.4)).into(),
        Cylinder::new(0.3, 0.3).into(),
        ConvexHull::rock(0.5).expect("rock").into(),
        Capsule::new(0.2, 0.4).into(),
    ];
    for a in &shapes {
        for b in &shapes {
            let transform = random.transform(0.7);
            let mut cache1 = PairCache::default();
            let mut cache2 = PairCache::default();
            for _ in 0..3 {
                let first = collide(a, b, transform, SPECULATIVE, &mut cache1);
                let second = collide(a, b, transform, SPECULATIVE, &mut cache2);
                assert_eq!(format!("{first:?}"), format!("{second:?}"));
                assert_eq!(cache1, cache2);
            }
        }
    }
}

#[test]
fn warm_caches_follow_moving_shapes() {
    let hull: Shape = ConvexHull::rock(0.5).expect("rock").rounded(0.05).into();
    let others: Vec<Shape> = vec![
        Sphere::new(0.2).into(),
        Capsule::new(0.1, 0.3).into(),
        Capsule::tapered(
            Vec3::new(0.0, -0.25, 0.0),
            0.15,
            Vec3::new(0.0, 0.25, 0.0),
            0.05,
        )
        .into(),
        Cylinder::new(0.2, 0.2).into(),
        Cuboid::new(Vec3::splat(0.2)).rounded(0.02).into(),
    ];
    let mut random = Random::new(61);
    for other in &others {
        let mut cache = PairCache::default();
        let start = random.transform(0.8);
        for step in 0..60 {
            let angle = step as f32 * 0.1;
            let transform = Transform::new(
                start.p + 0.4 * Vec3::new(angle.cos(), (1.3 * angle).sin(), angle.sin()),
                start.q * Quat::from_rotation_y(angle),
            );
            let warm = collide(&hull, other, transform, SPECULATIVE, &mut cache);
            let cold = run(&hull, other, transform);
            if cold.points().iter().any(|point| point.separation < -0.05) {
                continue;
            }
            assert_eq!(
                warm.count,
                cold.count,
                "{:?} {warm:?} {cold:?}",
                other.kind()
            );
            for (a, b) in warm.points().iter().zip(cold.points()) {
                assert!(
                    (a.separation - b.separation).abs() < 1.0e-3,
                    "{warm:?} {cold:?}"
                );
            }
        }
    }
}

fn deepest(manifold: &Manifold) -> f32 {
    manifold
        .points()
        .iter()
        .map(|point| point.separation)
        .fold(f32::MAX, f32::min)
}

#[test]
fn warm_box_pairs_match_cold_results() {
    let block: Shape = Cuboid::new(Vec3::new(0.4, 0.3, 0.5)).into();
    let others: Vec<Shape> = vec![
        Cuboid::new(Vec3::splat(0.35)).into(),
        Capsule::new(0.15, 0.3).into(),
    ];
    let mut random = Random::new(79);
    let mut checked = 0;
    for other in &others {
        for _ in 0..8 {
            let mut cache = PairCache::default();
            let start = random.transform(0.7);
            for step in 0..40 {
                let angle = step as f32 * 0.02;
                let transform = Transform::new(
                    start.p + 0.1 * Vec3::new(angle.cos(), angle.sin(), 0.5 * angle),
                    start.q * Quat::from_rotation_y(angle),
                );
                let warm = collide(&block, other, transform, SPECULATIVE, &mut cache);
                let cold = run(&block, other, transform);
                if cold.points().iter().any(|point| point.separation < -0.05) {
                    continue;
                }
                checked += 1;
                assert_eq!(warm.count, cold.count, "{warm:?} {cold:?}");
                if !warm.is_empty() {
                    assert!(
                        (deepest(&warm) - deepest(&cold)).abs() < 5.0e-3,
                        "{warm:?} {cold:?}"
                    );
                }
            }
        }
    }
    assert!(checked > 100, "{checked}");
}
