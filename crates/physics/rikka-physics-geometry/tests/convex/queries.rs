use rikka_physics_math::{Transform, Vec3};

use crate::convex::distance::random_hull;
use crate::support::{Random, close, close_vec};
use rikka_physics_geometry::{
    Capsule, Convex, ConvexHull, Cuboid, Cylinder, RayInput, Shape, ShapeGeometry, Sphere,
};

fn contains(shape: &Shape, point: Vec3) -> bool {
    match shape {
        Shape::Sphere(sphere) => point.distance(sphere.center) <= sphere.radius,
        Shape::Capsule(capsule) => capsule.contains(point),
        Shape::Cuboid(cuboid) => {
            let local = cuboid.transform.apply_inverse(point);
            local.abs().cmple(cuboid.half_extents).all()
        }
        Shape::Cylinder(cylinder) => cylinder.contains(point),
        Shape::Hull(hull) => hull
            .planes()
            .iter()
            .all(|plane| plane.separation(point) <= 0.0),
        Shape::Plane(plane) => plane.separation(point) <= 0.0,
        _ => false,
    }
}

fn marched(shape: &Shape, ray: &RayInput) -> Option<f32> {
    let steps = 4000;
    let mut previous = 0.0;
    for step in 1..=steps {
        let t = ray.max_fraction * step as f32 / steps as f32;
        if contains(shape, ray.origin + t * ray.translation) {
            let (mut low, mut high) = (previous, t);
            for _ in 0..40 {
                let middle = 0.5 * (low + high);
                if contains(shape, ray.origin + middle * ray.translation) {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            return Some(high);
        }
        previous = t;
    }
    None
}

fn shapes(random: &mut Random) -> Vec<Shape> {
    vec![
        Sphere::with_center(Vec3::new(0.1, 0.2, -0.1), 0.6).into(),
        Capsule::new(0.3, 0.5).into(),
        Capsule::tapered(
            Vec3::new(-0.2, 0.0, 0.1),
            0.5,
            Vec3::new(0.4, 0.6, 0.0),
            0.15,
        )
        .into(),
        Cuboid::with_transform(random.transform(0.3), Vec3::new(0.3, 0.5, 0.7)).into(),
        Cylinder::with_transform(random.transform(0.3), 0.5, 0.4).into(),
        random_hull(random, 20, 0.7).into(),
    ]
}

#[test]
fn ray_casts_match_marching() {
    let mut random = Random::new(61);
    for shape in shapes(&mut random) {
        let mut hits = 0;
        let mut grazing = 0;
        for _ in 0..300 {
            let origin = random.vector(2.5);
            if contains(&shape, origin) {
                let hit = shape
                    .ray_cast(&RayInput::new(origin, random.vector(1.0)))
                    .expect("inside");
                assert_eq!(hit.fraction, 0.0);
                continue;
            }
            let target = random.vector(0.5);
            let ray = RayInput::new(origin, 2.0 * (target - origin));
            let expected = marched(&shape, &ray);
            let hit = shape.ray_cast(&ray);
            match (expected, hit) {
                (Some(fraction), Some(hit)) => {
                    assert!(
                        close(hit.fraction, fraction, 2.0e-3),
                        "{:?} {} vs {fraction}",
                        shape.kind(),
                        hit.fraction
                    );
                    let point = ray.origin + hit.fraction * ray.translation;
                    assert!(close_vec(hit.point, point, 1.0e-3));
                    assert!((hit.normal.length() - 1.0).abs() < 1.0e-4);
                    assert!(hit.normal.dot(ray.translation) < 0.0);
                    hits += 1;
                }
                (None, None) => {}
                (expected, hit) => {
                    grazing += 1;
                    assert!(
                        grazing <= 2,
                        "{:?}: marched {expected:?}, cast {hit:?}",
                        shape.kind()
                    );
                }
            }
        }
        assert!(hits > 50, "{:?}", shape.kind());
    }
}

#[test]
fn point_projection_reaches_surface() {
    let mut random = Random::new(81);
    let hull = ConvexHull::cuboid(Vec3::splat(0.5), Transform::IDENTITY).expect("hull");
    let cuboid = Cuboid::new(Vec3::splat(0.5));
    let capsule = Capsule::tapered(Vec3::ZERO, 0.4, Vec3::new(0.0, 1.0, 0.0), 0.2);
    for _ in 0..100 {
        let point = random.vector(1.5);
        let from_hull = Convex::Hull(&hull).project_point(point).expect("hull");
        let from_box = Convex::Cuboid(&cuboid).project_point(point).expect("box");
        assert!(close(from_hull.distance, from_box.distance, 1.0e-3));
        let outside = (point.abs() - Vec3::splat(0.5)).max(Vec3::ZERO).length();
        if outside > 0.0 {
            assert!(close(from_box.distance, outside, 1.0e-4));
        } else {
            let inside = (Vec3::splat(0.5) - point.abs()).min_element();
            assert!(
                close(from_box.distance, -inside, 1.0e-3),
                "{} vs {}",
                from_box.distance,
                -inside
            );
        }
        let surface = Convex::Capsule(&capsule)
            .project_point(point)
            .expect("capsule");
        assert_eq!(capsule.contains(point), surface.distance <= 0.0);
        let back = surface.point + 1.0e-3 * surface.normal;
        assert!(!capsule.contains(back));
    }
}
