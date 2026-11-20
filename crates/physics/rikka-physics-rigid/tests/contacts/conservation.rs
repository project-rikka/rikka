use rikka_physics_collision::Material;
use rikka_physics_geometry::{Cuboid, Sphere};
use rikka_physics_math::{Mat3, Quat, Vec3};
use rikka_physics_rigid::{Body, RigidWorld, ShapeDef};

#[test]
fn elastic_collision_conserves_momentum_and_energy() {
    let mut world = RigidWorld::builder()
        .gravity(Vec3::ZERO)
        .restitution_threshold(0.0)
        .build()
        .unwrap();
    let bouncy = Material::default().restitution(1.0).friction(0.0);
    let a = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(-2.0, 0.0, 0.0))
                .linear_velocity(Vec3::new(3.0, 0.0, 0.0))
                .shape(ShapeDef::new(Sphere::new(0.5)).material(bouncy)),
        )
        .unwrap();
    let b = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(2.0, 0.1, 0.0))
                .linear_velocity(Vec3::new(-1.0, 0.0, 0.0))
                .shape(
                    ShapeDef::new(Sphere::new(0.5))
                        .material(bouncy)
                        .density(2000.0),
                ),
        )
        .unwrap();
    let momentum = |world: &RigidWorld| {
        world.linear_velocity(a).unwrap() * world.mass(a).unwrap()
            + world.linear_velocity(b).unwrap() * world.mass(b).unwrap()
    };
    let energy = |world: &RigidWorld| {
        0.5 * world.mass(a).unwrap() * world.linear_velocity(a).unwrap().length_squared()
            + 0.5 * world.mass(b).unwrap() * world.linear_velocity(b).unwrap().length_squared()
    };
    let p0 = momentum(&world);
    let e0 = energy(&world);
    for _ in 0..120 {
        world.step(1.0 / 60.0);
    }
    assert!(
        world.linear_velocity(a).unwrap().x < 0.0,
        "spheres never collided"
    );
    assert!((momentum(&world) - p0).length() < 1e-3 * p0.length().max(1.0));
    assert!(
        (energy(&world) - e0).abs() < 0.02 * e0,
        "energy {} -> {}",
        e0,
        energy(&world)
    );
}

#[test]
fn free_spin_conserves_energy_and_angular_momentum() {
    let mut world = RigidWorld::builder().gravity(Vec3::ZERO).build().unwrap();
    let body = world
        .create_body(
            Body::dynamic()
                .rotation(Quat::from_rotation_x(0.3))
                .angular_velocity(Vec3::new(4.0, 0.1, 0.2))
                .linear_velocity(Vec3::new(0.5, 0.0, 0.0))
                .shape(Cuboid::new(Vec3::new(0.2, 0.5, 1.0))),
        )
        .unwrap();
    let local = {
        let m = world.mass(body).unwrap();
        let (x, y, z) = (0.4f32, 1.0f32, 2.0f32);
        Mat3::from_diagonal(Vec3::new(y * y + z * z, x * x + z * z, x * x + y * y) * (m / 12.0))
    };
    let measure = |world: &RigidWorld| {
        let q = world.pose(body).unwrap().q;
        let w = world.angular_velocity(body).unwrap();
        let inertia = Mat3::from_quat(q) * local * Mat3::from_quat(q).transpose();
        let l = inertia * w;
        (0.5 * w.dot(l), l)
    };
    let (e0, l0) = measure(&world);
    for _ in 0..600 {
        world.step(1.0 / 60.0);
    }
    let (e1, l1) = measure(&world);
    assert!((e1 - e0).abs() < 0.02 * e0, "energy {e0} -> {e1}");
    assert!(
        (l1 - l0).length() < 0.01 * l0.length(),
        "angular momentum {l0} -> {l1}"
    );
    let velocity = world.linear_velocity(body).unwrap();
    assert!((velocity - Vec3::new(0.5, 0.0, 0.0)).length() < 1e-5);
}
