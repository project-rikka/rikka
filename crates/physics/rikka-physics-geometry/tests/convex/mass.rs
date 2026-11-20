use std::f32::consts::PI;

use rikka_physics_math::{Mat3, Quat, Transform, Vec3};

use crate::support::{close, close_vec};
use rikka_physics_geometry::{Capsule, Cuboid, MassData, ShapeGeometry, Sphere};

fn assert_mass(actual: MassData, expected: MassData, tolerance: f32) {
    assert!(
        close(actual.mass, expected.mass, tolerance),
        "{actual:?} != {expected:?}"
    );
    assert!(
        close_vec(actual.center, expected.center, tolerance),
        "{actual:?} != {expected:?}"
    );
    for column in 0..3 {
        assert!(
            close_vec(
                actual.inertia.col(column),
                expected.inertia.col(column),
                tolerance
            ),
            "{actual:?} != {expected:?}"
        );
    }
}

fn rotated(inertia: Mat3, rotation: Quat) -> Mat3 {
    let r = Mat3::from_quat(rotation);
    r * inertia * r.transpose()
}

#[test]
fn sphere_matches_formula() {
    let sphere = Sphere::with_center(Vec3::new(1.0, 2.0, 3.0), 0.5);
    let mass = 2.0 * 4.0 / 3.0 * PI * 0.125;
    let expected = MassData {
        mass,
        center: sphere.center,
        inertia: Mat3::from_diagonal(Vec3::splat(0.4 * mass * 0.25)),
    };
    assert_mass(sphere.mass(2.0), expected, 1.0e-5);
}

fn sliced_capsule(r1: f32, r2: f32, length: f32) -> MassData {
    let (r1, r2, length) = (f64::from(r1), f64::from(r2), f64::from(length));
    let sine = (r1 - r2) / length;
    let cosine = (1.0 - sine * sine).sqrt();
    let (z0, z1) = (r1 * sine, length + r2 * sine);
    let radius = |z: f64| {
        let mut best: f64 = 0.0;
        if z.abs() <= r1 {
            best = best.max((r1 * r1 - z * z).sqrt());
        }
        if (z - length).abs() <= r2 {
            best = best.max((r2 * r2 - (z - length).powi(2)).sqrt());
        }
        if (z0..=z1).contains(&z) {
            best = best.max(r1 * cosine + (r2 - r1) * cosine * (z - z0) / (z1 - z0));
        }
        best
    };
    let steps = 200_000;
    let (low, high) = (-r1, length + r2);
    let dz = (high - low) / f64::from(steps);
    let (mut volume, mut first, mut second, mut quartic) = (0.0, 0.0, 0.0, 0.0);
    for step in 0..steps {
        let z = low + (f64::from(step) + 0.5) * dz;
        let rho = radius(z);
        let area = std::f64::consts::PI * rho * rho;
        volume += area * dz;
        first += area * z * dz;
        second += area * z * z * dz;
        quartic += area * rho * rho * dz;
    }
    let center = first / volume;
    let axial = 0.5 * quartic;
    let transverse = 0.25 * quartic + second - volume * center * center;
    MassData {
        mass: volume as f32,
        center: Vec3::new(0.0, center as f32, 0.0),
        inertia: Mat3::from_diagonal(Vec3::new(
            transverse as f32,
            axial as f32,
            transverse as f32,
        )),
    }
}

#[test]
fn tapered_capsule_matches_slicing() {
    for (r1, r2, length) in [
        (0.5, 0.2, 1.0),
        (0.1, 0.4, 0.8),
        (0.3, 0.3, 0.5),
        (0.6, 0.05, 2.0),
    ] {
        let capsule = Capsule::tapered(Vec3::ZERO, r1, Vec3::new(0.0, length, 0.0), r2);
        assert_mass(capsule.mass(1.0), sliced_capsule(r1, r2, length), 2.0e-4);
    }
}

#[test]
fn cuboid_matches_formula_and_rotates() {
    let h = Vec3::new(0.5, 1.0, 1.5);
    let mass = 8.0 * h.x * h.y * h.z;
    let local = Mat3::from_diagonal(
        mass / 3.0
            * Vec3::new(
                h.y * h.y + h.z * h.z,
                h.x * h.x + h.z * h.z,
                h.x * h.x + h.y * h.y,
            ),
    );
    let rotation =
        Quat::from_rotation_x(0.3) * Quat::from_rotation_y(0.9) * Quat::from_rotation_z(-0.4);
    let transform = Transform::new(Vec3::new(1.0, -2.0, 0.5), rotation);
    let expected = MassData {
        mass,
        center: transform.p,
        inertia: rotated(local, rotation),
    };
    assert_mass(
        Cuboid::with_transform(transform, h).mass(1.0),
        expected,
        1.0e-5,
    );
}
