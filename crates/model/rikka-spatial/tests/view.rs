use glam::camera::rh::proj::{opengl, vulkan};
use glam::camera::rh::view::look_at_mat4;
use glam::{Mat4, Vec2, Vec3};
use rikka_spatial::{Aabb, ClipSpace, Containment, View, Viewport};

use crate::support::Rng;

fn views() -> [View; 4] {
    let viewport = Viewport::new(Vec2::new(40.0, 20.0), Vec2::new(800.0, 600.0));
    let eye = look_at_mat4(Vec3::new(3.0, -4.0, 2.5), Vec3::new(0.0, 0.0, 0.5), Vec3::Z);
    let aspect = 800.0 / 600.0;
    [
        (
            vulkan::perspective_infinite_reverse(0.9, aspect, 0.05) * eye,
            ClipSpace::VULKAN_REVERSED_Z,
        ),
        (
            opengl::perspective(0.9, aspect, 0.1, 100.0) * eye,
            ClipSpace::OPENGL,
        ),
        (
            opengl::orthographic(-3.0, 3.0, -2.0, 2.0, 0.1, 50.0) * eye,
            ClipSpace::OPENGL,
        ),
        (
            vulkan::perspective(0.9, aspect, 0.1, 100.0) * eye,
            ClipSpace::VULKAN,
        ),
    ]
    .map(|(matrix, clip)| View::new(matrix, viewport, clip).unwrap())
}

#[test]
fn pick_rays_pass_through_projected_points() {
    let mut rng = Rng::new(50);
    for view in views() {
        for _ in 0..500 {
            let point = rng.vec3(-1.5, 1.5);
            let projected = view.project(point).unwrap();
            let ray = view.pick_ray(projected.position).unwrap();
            let along = (point - ray.origin()).dot(ray.direction());
            assert!(along > 0.0);
            let distance = (ray.origin() + ray.direction() * along).distance(point);
            assert!(distance < 2e-4 * along.max(1.0), "{distance}");
            let unprojected = view.unproject(projected.position, projected.depth).unwrap();
            assert!(unprojected.distance(point) < 1e-3);
        }
    }
}

#[test]
fn perspective_detection_and_ray_origins() {
    let [reversed, gl, ortho, vulkan] = views();
    assert!(
        reversed.is_perspective()
            && gl.is_perspective()
            && vulkan.is_perspective()
            && !ortho.is_perspective()
    );
    let eye = Vec3::new(3.0, -4.0, 2.5);
    for view in [reversed, gl, vulkan] {
        let corner = view.pick_ray(Vec2::new(40.0, 20.0)).unwrap();
        let center = view.pick_ray(Vec2::new(440.0, 320.0)).unwrap();
        let towards = (Vec3::new(0.0, 0.0, 0.5) - eye).normalize();
        assert!(center.direction().dot(towards) > 0.9999);
        assert!(corner.origin().distance(eye) < 0.2);
    }
    let first = ortho.pick_ray(Vec2::new(100.0, 100.0)).unwrap();
    let second = ortho.pick_ray(Vec2::new(700.0, 500.0)).unwrap();
    assert!(first.direction().dot(second.direction()) > 0.99999);
}

#[test]
fn region_frustum_selects_points_projecting_inside_the_rectangle() {
    let mut rng = Rng::new(51);
    for view in views() {
        let corner = Vec2::new(300.0, 200.0);
        let opposite = Vec2::new(150.0, 450.0);
        let frustum = view.region_frustum(corner, opposite);
        let full = view.frustum();
        for _ in 0..3_000 {
            let point = rng.vec3(-3.0, 3.0);
            let inside_full = full.contains_point(point);
            let expected = inside_full
                && view.project(point).is_some_and(|projected| {
                    let position = projected.position;
                    position.cmpge(corner.min(opposite) + 1e-2).all()
                        && position.cmple(corner.max(opposite) - 1e-2).all()
                });
            let outside = !inside_full
                || view.project(point).is_none_or(|projected| {
                    let position = projected.position;
                    position.cmplt(corner.min(opposite) - 1e-2).any()
                        || position.cmpgt(corner.max(opposite) + 1e-2).any()
                });
            if expected {
                assert!(frustum.contains_point(point));
            }
            if outside {
                assert!(!frustum.contains_point(point));
            }
        }
    }
}

#[test]
fn screen_bounds_and_segments_respect_the_near_plane() {
    let [reversed, ..] = views();
    let bounds = Aabb::new(Vec3::splat(-0.25), Vec3::splat(0.25));
    let center = reversed.project(Vec3::ZERO).unwrap().position;
    assert_eq!(reversed.screen_distance_bound(&bounds, center), 0.0);
    let far = center + Vec2::new(300.0, 0.0);
    let bound = reversed.screen_distance_bound(&bounds, far);
    let corners_distance = bounds
        .corners()
        .iter()
        .map(|corner| reversed.project(*corner).unwrap().position.distance(far))
        .fold(f32::INFINITY, f32::min);
    assert!(bound > 0.0 && bound <= corners_distance + 1e-3);
    let eye = Vec3::new(3.0, -4.0, 2.5);
    let around_eye = Aabb::new(eye - Vec3::splat(0.5), eye + Vec3::splat(0.5));
    assert_eq!(reversed.screen_distance_bound(&around_eye, far), 0.0);
    let segment = reversed
        .project_segment(Vec3::ZERO, eye + (eye - Vec3::ZERO))
        .unwrap();
    assert!(segment.start.distance(center) < 1e-3);
    assert!(
        reversed
            .project_segment(eye + Vec3::X * 2.0 + (eye - Vec3::ZERO), eye * 2.0)
            .is_none()
    );
    let classified = reversed.frustum().classify_aabb(&bounds);
    assert_eq!(classified, Containment::Inside);
    assert!(
        View::new(
            Mat4::ZERO,
            Viewport::from_size(Vec2::ONE),
            ClipSpace::OPENGL
        )
        .is_none()
    );
}
