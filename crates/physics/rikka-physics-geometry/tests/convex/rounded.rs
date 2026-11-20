use rikka_physics_math::{Aabb, Mat3, Quat, Transform, Vec3};

use crate::support::{Random, close};
use rikka_physics_geometry::{
    ConvexHull, Cuboid, DistanceInput, MassData, RayInput, Shape, ShapeGeometry, ShapeProxy,
    SimplexCache, Sphere, shape_distance,
};

fn core_distance(shape: &Shape, point: Vec3) -> f32 {
    let target = [point];
    let proxy = ShapeProxy::new(&target, 0.0);
    let mut cache = SimplexCache::default();
    match shape {
        Shape::Cuboid(cuboid) => {
            shape_distance(
                &DistanceInput::new(&cuboid.rounded(0.0), &proxy, Transform::IDENTITY),
                &mut cache,
            )
            .distance
        }
        Shape::Hull(hull) => {
            let core = hull.clone().rounded(0.0);
            shape_distance(
                &DistanceInput::new(&core, &proxy, Transform::IDENTITY),
                &mut cache,
            )
            .distance
        }
        _ => f32::MAX,
    }
}

fn radius(shape: &Shape) -> f32 {
    match shape {
        Shape::Cuboid(cuboid) => cuboid.radius,
        Shape::Hull(hull) => hull.radius(),
        _ => 0.0,
    }
}

fn contains(shape: &Shape, point: Vec3) -> bool {
    core_distance(shape, point) <= radius(shape)
}

fn sampled(shape: &Shape, bounds: Aabb, steps: usize) -> MassData {
    let size = bounds.size() / steps as f32;
    let cell = f64::from(size.x * size.y * size.z);
    let (mut volume, mut first, mut second) = (0.0f64, [0.0f64; 3], [[0.0f64; 3]; 3]);
    for i in 0..steps {
        for j in 0..steps {
            for k in 0..steps {
                let p =
                    bounds.min + size * Vec3::new(i as f32 + 0.5, j as f32 + 0.5, k as f32 + 0.5);
                if !contains(shape, p) {
                    continue;
                }
                let q = p.as_dvec3();
                volume += cell;
                for a in 0..3 {
                    first[a] += cell * q[a];
                    for b in 0..3 {
                        second[a][b] += cell * q[a] * q[b];
                    }
                }
            }
        }
    }
    let center = first.map(|value| value / volume);
    let central = |a: usize, b: usize| second[a][b] - volume * center[a] * center[b];
    let trace = central(0, 0) + central(1, 1) + central(2, 2);
    let inertia: [[f64; 3]; 3] = std::array::from_fn(|a| {
        std::array::from_fn(|b| if a == b { trace } else { 0.0 } - central(a, b))
    });
    let column = |b: usize| {
        Vec3::new(
            inertia[0][b] as f32,
            inertia[1][b] as f32,
            inertia[2][b] as f32,
        )
    };
    MassData {
        mass: volume as f32,
        center: Vec3::new(center[0] as f32, center[1] as f32, center[2] as f32),
        inertia: Mat3::from_cols(column(0), column(1), column(2)),
    }
}

fn assert_mass(actual: MassData, expected: MassData, tolerance: f32) {
    let scale = expected.inertia.x_axis.x.abs()
        + expected.inertia.y_axis.y.abs()
        + expected.inertia.z_axis.z.abs();
    assert!(
        close(actual.mass, expected.mass, tolerance),
        "{actual:?} != {expected:?}"
    );
    assert!(
        (actual.center - expected.center).length() <= tolerance,
        "{actual:?} != {expected:?}"
    );
    for column in 0..3 {
        let difference = (actual.inertia.col(column) - expected.inertia.col(column)).length();
        assert!(
            difference <= tolerance * scale,
            "{actual:?} != {expected:?}"
        );
    }
}

#[test]
fn rounded_box_mass_matches_sampling() {
    let transform = Transform::new(
        Vec3::new(0.1, -0.2, 0.3),
        Quat::from_rotation_z(0.4) * Quat::from_rotation_x(0.7),
    );
    let cuboid = Cuboid::with_transform(transform, Vec3::new(0.4, 0.2, 0.6)).rounded(0.15);
    let shape: Shape = cuboid.into();
    let expected = sampled(&shape, shape.aabb(Transform::IDENTITY), 90);
    assert_mass(cuboid.mass(1.0), expected, 5.0e-3);
    assert_mass(
        Cuboid::new(Vec3::ZERO).rounded(0.5).mass(2.0),
        Sphere::new(0.5).mass(2.0),
        1.0e-5,
    );
}

#[test]
fn rounded_rays_match_marching() {
    let mut random = Random::new(91);
    let shapes: Vec<Shape> = vec![
        Cuboid::with_transform(random.transform(0.2), Vec3::new(0.3, 0.4, 0.2))
            .rounded(0.2)
            .into(),
        ConvexHull::rock(0.5).expect("rock").rounded(0.1).into(),
    ];
    for shape in &shapes {
        let mut hits = 0;
        for _ in 0..150 {
            let origin = random.vector(2.0);
            let target = random.vector(0.4);
            let ray = RayInput::new(origin, 2.0 * (target - origin));
            if contains(shape, origin) {
                assert_eq!(shape.ray_cast(&ray).expect("inside").fraction, 0.0);
                continue;
            }
            let hit = shape.ray_cast(&ray);
            let steps = 600;
            let entry = (1..=steps)
                .map(|step| step as f32 / steps as f32)
                .find(|&t| contains(shape, origin + t * ray.translation));
            match (entry, hit) {
                (Some(t), Some(hit)) => {
                    assert!(
                        hit.fraction <= t + 1.0e-4 && hit.fraction >= t - 2.0 / steps as f32,
                        "{} vs {t}",
                        hit.fraction
                    );
                    let distance = core_distance(shape, hit.point);
                    assert!((distance - radius(shape)).abs() < 1.0e-3);
                    assert!(hit.normal.dot(ray.translation) < 0.0);
                    hits += 1;
                }
                (None, None) => {}
                (entry, hit) => assert!(
                    hit.is_some_and(|hit| hit.fraction > 0.99) || entry.is_some_and(|t| t > 0.99),
                    "{entry:?} {hit:?}"
                ),
            }
        }
        assert!(hits > 30);
    }
}
