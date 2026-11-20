use glam::{Vec2, Vec3};
use rikka_spatial::{
    Aabb, Culling, Plane, Ray, intersect_ray_aabb, intersect_ray_plane, intersect_ray_sphere,
    intersect_ray_triangle,
};

use crate::support::{Rng, grid_mesh, ray_triangle_reference};

#[test]
fn ray_triangle_matches_reference_on_random_cases() {
    let mut rng = Rng::new(10);
    let mut hits = 0;
    for _ in 0..50_000 {
        let triangle = [
            rng.vec3(-1.0, 1.0),
            rng.vec3(-1.0, 1.0),
            rng.vec3(-1.0, 1.0),
        ];
        let origin = rng.vec3(-3.0, 3.0);
        let target = rng.vec3(-1.0, 1.0);
        let ray = Ray::new(origin, target - origin).unwrap();
        let expected = ray_triangle_reference(origin, ray.direction(), triangle);
        let actual = intersect_ray_triangle(&ray, triangle, f32::INFINITY, Culling::None);
        match (expected, actual) {
            (Some(distance), Some(hit)) => {
                hits += 1;
                assert!((f64::from(hit.distance) - distance).abs() < 1e-4 * distance.max(1.0));
                let [a, b, c] = triangle;
                let point = a * hit.barycentric.x + b * hit.barycentric.y + c * hit.barycentric.z;
                assert!(point.distance(ray.at(hit.distance)) < 1e-4);
                assert!((hit.barycentric.element_sum() - 1.0).abs() < 1e-5);
            }
            (None, None) => {}
            (expected, actual) => {
                let [a, b, c] = triangle;
                let area = (b - a).cross(c - a).length();
                let near_edge = actual.is_some_and(|hit| hit.barycentric.min_element() < 1e-4);
                assert!(
                    near_edge || area < 1e-4 || expected.is_some(),
                    "mismatch {expected:?} {actual:?}"
                );
            }
        }
    }
    assert!(hits > 1_000);
}

#[test]
fn watertight_rays_never_slip_between_shared_edges() {
    let mesh = grid_mesh(16, 16, |u, v| Vec3::new(u * 4.0 - 2.0, v * 4.0 - 2.0, 0.0));
    let mut rng = Rng::new(11);
    for _ in 0..4_000 {
        let column = rng.index(15) as f32 + 1.0;
        let row = rng.index(16) as f32 + rng.unit();
        let along_edge = Vec2::new(column / 16.0 * 4.0 - 2.0, row / 16.0 * 4.0 - 2.0);
        let target = match rng.index(3) {
            0 => along_edge,
            1 => Vec2::new(along_edge.y, along_edge.x),
            _ => Vec2::new(
                column / 16.0 * 4.0 - 2.0,
                (rng.index(15) as f32 + 1.0) / 16.0 * 4.0 - 2.0,
            ),
        };
        let origin = Vec3::new(
            target.x + rng.range(-0.5, 0.5),
            target.y + rng.range(-0.5, 0.5),
            5.0,
        );
        let ray = Ray::new(origin, Vec3::new(target.x, target.y, 0.0) - origin).unwrap();
        let hit = (0..mesh.triangles.len()).any(|index| {
            intersect_ray_triangle(&ray, mesh.triangle(index), f32::INFINITY, Culling::None)
                .is_some()
        });
        assert!(hit, "ray through {target} slipped between triangles");
    }
}

#[test]
fn culling_rejects_the_chosen_side() {
    let triangle = [Vec3::ZERO, Vec3::X, Vec3::Y];
    let down = Ray::new(Vec3::new(0.2, 0.2, 1.0), Vec3::NEG_Z).unwrap();
    let up = Ray::new(Vec3::new(0.2, 0.2, -1.0), Vec3::Z).unwrap();
    let cases = [
        (&down, Culling::None, true),
        (&down, Culling::Back, true),
        (&down, Culling::Front, false),
        (&up, Culling::None, true),
        (&up, Culling::Back, false),
        (&up, Culling::Front, true),
    ];
    for (ray, culling, expected) in cases {
        assert_eq!(
            intersect_ray_triangle(ray, triangle, f32::INFINITY, culling).is_some(),
            expected,
            "{culling:?}"
        );
    }
    let hit = intersect_ray_triangle(&down, triangle, f32::INFINITY, Culling::None).unwrap();
    assert!((hit.distance - 1.0).abs() < 1e-6);
    assert!((hit.barycentric - Vec3::new(0.6, 0.2, 0.2)).length() < 1e-6);
}

#[test]
fn ray_triangle_respects_limits_and_degeneracy() {
    let triangle = [Vec3::ZERO, Vec3::X, Vec3::Y];
    let ray = Ray::new(Vec3::new(0.2, 0.2, 1.0), Vec3::NEG_Z).unwrap();
    assert!(intersect_ray_triangle(&ray, triangle, 0.99, Culling::None).is_none());
    assert!(intersect_ray_triangle(&ray, triangle, 1.0, Culling::None).is_some());
    let behind = Ray::new(Vec3::new(0.2, 0.2, 1.0), Vec3::Z).unwrap();
    assert!(intersect_ray_triangle(&behind, triangle, f32::INFINITY, Culling::None).is_none());
    let sliver = [Vec3::ZERO, Vec3::X, Vec3::X * 2.0];
    let grazing = Ray::new(Vec3::new(0.5, 1.0, 0.0), Vec3::NEG_Y).unwrap();
    assert!(intersect_ray_triangle(&ray, sliver, f32::INFINITY, Culling::None).is_none());
    assert!(intersect_ray_triangle(&grazing, sliver, f32::INFINITY, Culling::None).is_none());
    let parallel = Ray::new(Vec3::new(-1.0, 0.2, 0.0), Vec3::X).unwrap();
    assert!(intersect_ray_triangle(&parallel, triangle, f32::INFINITY, Culling::None).is_none());
    let corner = Ray::new(Vec3::new(1.0, 0.0, 1.0), Vec3::NEG_Z).unwrap();
    let hit = intersect_ray_triangle(&corner, triangle, f32::INFINITY, Culling::None).unwrap();
    assert!((hit.barycentric - Vec3::Y).length() < 1e-6);
}

#[test]
fn ray_aabb_matches_analytic_intervals() {
    let bounds = Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0));
    let cases = [
        (Vec3::new(-3.0, 0.0, 0.0), Vec3::X, Some((2.0, 4.0))),
        (Vec3::ZERO, Vec3::X, Some((0.0, 1.0))),
        (Vec3::new(-3.0, 2.0, 0.0), Vec3::X, None),
        (Vec3::new(-3.0, 1.0, 0.0), Vec3::X, Some((2.0, 4.0))),
        (Vec3::new(-1.0, -3.0, 0.5), Vec3::Y, Some((2.0, 4.0))),
        (Vec3::new(3.0, 0.0, 0.0), Vec3::X, None),
        (
            Vec3::new(-3.0, -3.0, -3.0),
            Vec3::ONE,
            Some((2.0 * 3f32.sqrt(), 4.0 * 3f32.sqrt())),
        ),
    ];
    for (origin, direction, expected) in cases {
        let ray = Ray::new(origin, direction).unwrap();
        let interval = intersect_ray_aabb(&ray, &bounds, f32::INFINITY);
        match expected {
            None => assert!(interval.is_none(), "{origin} {direction}"),
            Some((enter, exit)) => {
                let interval = interval.unwrap();
                assert!(
                    (interval.enter - enter).abs() < 1e-5 && (interval.exit - exit).abs() < 1e-4,
                    "{origin}"
                );
            }
        }
    }
    let ray = Ray::new(Vec3::new(-3.0, 0.0, 0.0), Vec3::X).unwrap();
    assert!(intersect_ray_aabb(&ray, &bounds, 1.5).is_none());
    assert!(intersect_ray_aabb(&ray, &Aabb::EMPTY, f32::INFINITY).is_none());
}

#[test]
fn ray_plane_and_sphere_match_analytic_results() {
    let plane = Plane::from_point_normal(Vec3::new(0.0, 0.0, 2.0), Vec3::Z).unwrap();
    let ray = Ray::new(Vec3::ZERO, Vec3::new(0.0, 3.0, 4.0)).unwrap();
    assert!((intersect_ray_plane(&ray, &plane, f32::INFINITY).unwrap() - 2.5).abs() < 1e-6);
    assert!(intersect_ray_plane(&ray, &plane, 2.0).is_none());
    assert!(
        intersect_ray_plane(
            &Ray::new(Vec3::ZERO, Vec3::X).unwrap(),
            &plane,
            f32::INFINITY
        )
        .is_none()
    );
    assert!(
        intersect_ray_plane(
            &Ray::new(Vec3::ZERO, Vec3::NEG_Z).unwrap(),
            &plane,
            f32::INFINITY
        )
        .is_none()
    );

    let outside = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::X).unwrap();
    let interval = intersect_ray_sphere(&outside, Vec3::ZERO, 2.0, f32::INFINITY).unwrap();
    assert!((interval.enter - 3.0).abs() < 1e-6 && (interval.exit - 7.0).abs() < 1e-6);
    let inside = Ray::new(Vec3::ZERO, Vec3::Y).unwrap();
    let interval = intersect_ray_sphere(&inside, Vec3::ZERO, 2.0, f32::INFINITY).unwrap();
    assert!(interval.enter == 0.0 && (interval.exit - 2.0).abs() < 1e-6);
    let tangent = Ray::new(Vec3::new(-5.0, 2.0, 0.0), Vec3::X).unwrap();
    let interval = intersect_ray_sphere(&tangent, Vec3::ZERO, 2.0, f32::INFINITY).unwrap();
    assert!((interval.enter - 5.0).abs() < 1e-3);
    assert!(
        intersect_ray_sphere(
            &Ray::new(Vec3::new(-5.0, 2.1, 0.0), Vec3::X).unwrap(),
            Vec3::ZERO,
            2.0,
            f32::INFINITY
        )
        .is_none()
    );
    let far = Ray::new(Vec3::new(-1e4, 0.0, 0.0), Vec3::X).unwrap();
    let interval = intersect_ray_sphere(&far, Vec3::ZERO, 1e-2, f32::INFINITY).unwrap();
    assert!((interval.enter - (1e4 - 1e-2)).abs() < 1e-2);
    assert!(Ray::new(Vec3::ZERO, Vec3::ZERO).is_none());
}
