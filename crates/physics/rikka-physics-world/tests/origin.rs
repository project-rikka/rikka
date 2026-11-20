use rikka_physics_world::{Body, BodyId, Cuboid, Plane, Vec3, World};

fn scene() -> (World, BodyId) {
    let mut world = World::builder().build().unwrap();
    world.create_body(Body::fixed().shape(Plane::up())).unwrap();
    let crate_box = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 0.5, 0.0))
                .shape(Cuboid::new(Vec3::splat(0.1))),
        )
        .unwrap();
    (world, crate_box)
}

#[test]
fn origin_shift_moves_bodies() {
    let offset = Vec3::new(10.0, 0.0, -4.0);
    let (mut plain, plain_box) = scene();
    let (mut shifted, shifted_box) = scene();
    for frame in 0..60 {
        if frame == 20 {
            shifted.shift_origin(offset).unwrap();
        }
        plain.advance(1.0 / 60.0);
        shifted.advance(1.0 / 60.0);
    }
    let expected = plain.latest_pose(plain_box).unwrap().p - offset;
    let actual = shifted.latest_pose(shifted_box).unwrap().p;
    assert!(
        (expected - actual).length() < 1e-3,
        "{expected} vs {actual}"
    );
}
