use rikka_physics_math::{Aabb, Mat3, Quat, Transform, Vec3, quat_between, quat_from_basis};

#[test]
fn rotations_map_vectors_and_bases_round_trip() {
    let pairs = [
        (Vec3::X, Vec3::Y),
        (Vec3::X, -Vec3::X),
        (
            Vec3::new(0.3, 0.4, -0.2).normalize(),
            Vec3::new(-0.8, 0.1, 0.3).normalize(),
        ),
    ];
    for (from, to) in pairs {
        assert!((quat_between(from, to) * from - to).length() < 1e-5);
    }
    let q = Quat::from_euler(glam::EulerRot::XYZ, 0.4, -2.1, 2.9);
    assert!(quat_from_basis(Mat3::from_quat(q)).dot(q).abs() > 1.0 - 1e-6);
}

#[test]
fn transformed_bounds_enclose_corners_and_rays_hit_front() {
    let bounds = Aabb::new(Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0));
    let transform = Transform::new(Vec3::new(5.0, 0.0, 0.0), Quat::from_rotation_y(0.7));
    let world = bounds.transformed(transform);
    for i in 0..8 {
        let corner = Vec3::new(
            if i & 1 == 0 { -1.0 } else { 1.0 },
            if i & 2 == 0 { -2.0 } else { 2.0 },
            if i & 4 == 0 { -3.0 } else { 3.0 },
        );
        assert!(world.inflated(1e-5).contains_point(transform.apply(corner)));
    }

    let unit = Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0));
    let along = Vec3::new(4.0, 0.0, 0.0);
    assert_eq!(
        unit.ray_fraction(Vec3::new(-3.0, 0.0, 0.0), along, 1.0),
        Some(0.5)
    );
    assert!(
        unit.ray_fraction(Vec3::new(-3.0, 2.0, 0.0), along, 1.0)
            .is_none()
    );
}
