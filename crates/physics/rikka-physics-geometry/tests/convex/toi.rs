use rikka_physics_math::{Transform, Vec3};

use crate::support::close;
use rikka_physics_geometry::{Cuboid, Sphere, Sweep, ToiInput, ToiState, time_of_impact};

#[test]
fn fast_sphere_stops_at_thin_wall() {
    let wall = Cuboid::new(Vec3::new(0.01, 2.0, 2.0));
    let bullet = Sphere::new(0.1);
    let sweep_wall = Sweep::fixed(Transform::IDENTITY, Vec3::ZERO);
    let sweep_bullet = Sweep::from_transforms(
        Transform::from_translation(Vec3::new(-5.0, 0.3, 0.0)),
        Transform::from_translation(Vec3::new(5.0, 0.3, 0.0)),
        Vec3::ZERO,
    );
    let output = time_of_impact(&ToiInput::new(&wall, sweep_wall, &bullet, sweep_bullet));
    assert_eq!(output.state, ToiState::Hit);
    let expected = (5.0 - 0.01 - 0.095) / 10.0;
    assert!(close(output.fraction, expected, 1.0e-3), "{output:?}");
    assert!(output.normal.dot(Vec3::X) < -0.99 || output.normal.dot(Vec3::X) > 0.99);
}
