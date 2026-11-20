use rikka_physics_world::{
    AsyncWorld, Body, BodyId, Cuboid, Plane, Sphere, Transform, Vec3, World,
};

fn scene() -> (World, BodyId) {
    let mut world = World::builder().workers(2).build().unwrap();
    world.create_body(Body::fixed().shape(Plane::up())).unwrap();
    let head = world
        .create_body(
            Body::kinematic()
                .position(Vec3::new(0.0, 1.0, 0.0))
                .shape(Sphere::new(0.1)),
        )
        .unwrap();
    for level in 0..3 {
        world
            .create_body(
                Body::dynamic()
                    .position(Vec3::new(0.5, 0.1 + 0.2 * level as f32, 0.0))
                    .shape(Cuboid::new(Vec3::splat(0.1))),
            )
            .unwrap();
    }
    (world, head)
}

fn drive(world: &mut World, head: BodyId, frame: u32) {
    let x = 0.2 * (frame as f32 * 0.2).sin();
    world
        .move_to(head, Transform::from_translation(Vec3::new(x, 1.0, 0.0)))
        .unwrap();
}

#[test]
fn async_stepping_matches_inline() {
    let (mut inline, head) = scene();
    let (world, _) = scene();
    let mut stepped = AsyncWorld::new(world).unwrap();
    for frame in 0..60 {
        drive(&mut inline, head, frame);
        inline.advance(1.0 / 60.0);

        drive(stepped.world().unwrap(), head, frame);
        stepped.step(1.0 / 60.0).unwrap();
        let shown = stepped.snapshot();
        assert!(shown.time() <= inline.time());
    }
    stepped.wait().unwrap();
    assert!(stepped.poll().unwrap());
    let snapshot = stepped.snapshot().clone();
    assert_eq!(snapshot.state_hash(), inline.state_hash());
    assert_eq!(snapshot.time(), inline.time());
    assert_eq!(snapshot.pose(head), Some(inline.pose(head).unwrap()));
    let world = stepped.into_world().unwrap();
    assert_eq!(world.state_hash(), inline.state_hash());
}
