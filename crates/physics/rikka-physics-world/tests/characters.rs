use rikka_physics_world::{
    Body, BodyId, CharacterDef, CharacterId, Cuboid, Plane, Recording, Vec3, World,
};

fn scene(world: &mut World) -> (CharacterId, BodyId) {
    world.create_body(Body::fixed().shape(Plane::up())).unwrap();
    let crate_box = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(1.0, 0.2, 0.0))
                .shape(Cuboid::new(Vec3::splat(0.2)))
                .mass(5.0),
        )
        .unwrap();
    let walker = world
        .create_character(
            CharacterDef::capsule(0.3, 1.8, Vec3::new(-1.0, 0.0, 0.0)).step_height(0.1),
        )
        .unwrap();
    (walker, crate_box)
}

fn walk(world: &mut World, walker: CharacterId, frames: u32) {
    for _ in 0..frames {
        world
            .move_character(walker, Vec3::new(1.5, -9.81, 0.0), 1.0 / 60.0)
            .unwrap();
        world.advance(1.0 / 60.0);
    }
}

#[test]
fn character_motion_is_recorded_and_replayed() {
    let mut world = World::builder().build().unwrap();
    world.start_recording().unwrap();
    let (walker, _) = scene(&mut world);
    walk(&mut world, walker, 90);
    let recording = world.finish_recording().unwrap();
    let decoded = Recording::from_bytes(&recording.to_bytes()).unwrap();
    let mut replayer = decoded.replayer().unwrap();
    while replayer.next_frame().unwrap() {}
    assert_eq!(replayer.report().divergence, None);
    assert_eq!(
        replayer.world().character(walker).unwrap().position(),
        world.character(walker).unwrap().position()
    );
}
