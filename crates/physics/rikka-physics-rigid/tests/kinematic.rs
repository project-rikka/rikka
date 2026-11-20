use rikka_physics_geometry::Cuboid;
use rikka_physics_math::{Transform, Vec3};
use rikka_physics_rigid::{Body, RigidWorld};

#[test]
fn move_to_sets_velocity_and_carries_riders() {
    let mut world = RigidWorld::builder().build().unwrap();
    let platform = world
        .create_body(Body::kinematic().shape(Cuboid::new(Vec3::new(3.0, 0.2, 3.0))))
        .unwrap();
    let rider = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 0.7, 0.0))
                .shape(Cuboid::new(Vec3::splat(0.5))),
        )
        .unwrap();
    for _ in 0..30 {
        world.step(1.0 / 60.0);
    }
    let dt = 1.0 / 60.0;
    let speed = 1.5;
    for step in 1..=90 {
        let target = Transform::from_translation(Vec3::new(speed * dt * step as f32, 0.0, 0.0));
        world.move_to(platform, target).unwrap();
        world.step(dt);
        let pose = world.pose(platform).unwrap();
        assert!(
            (pose.p - target.p).length() < 1e-4,
            "platform missed its target"
        );
        assert!((world.linear_velocity(platform).unwrap().x - speed).abs() < 1e-3);
    }
    let rider_x = world.pose(rider).unwrap().p.x;
    assert!(rider_x > 1.8 && rider_x < speed * 1.5, "rider at {rider_x}");
    assert!((world.linear_velocity(rider).unwrap().x - speed).abs() < 0.05);
    assert!(world.is_awake(rider).unwrap());
}
