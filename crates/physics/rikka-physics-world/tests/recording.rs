use rikka_physics_world::{
    Body, BodyId, Cuboid, D6, Error, Motion, Plane, Recording, Sphere, Transform, Vec3, World,
};

struct Scene {
    head: BodyId,
    crate_box: BodyId,
}

fn populate(world: &mut World) -> Scene {
    world.create_body(Body::fixed().shape(Plane::up())).unwrap();
    let head = world
        .create_body(
            Body::kinematic()
                .position(Vec3::new(0.0, 1.0, 0.0))
                .shape(Sphere::new(0.1)),
        )
        .unwrap();
    let crate_box = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.5, 0.3, 0.0))
                .shape(Cuboid::new(Vec3::splat(0.1))),
        )
        .unwrap();
    let bob = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 1.0, 0.25))
                .shape(Sphere::new(0.03)),
        )
        .unwrap();
    world
        .create_joint(
            D6::new(head, bob)
                .frame(Transform::from_translation(Vec3::new(0.0, 1.0, 0.12)))
                .linear(Motion::Locked),
        )
        .unwrap();
    Scene { head, crate_box }
}

fn drive(world: &mut World, scene: &Scene, frame: u32) {
    let x = 0.2 * (frame as f32 * 0.1).sin();
    world
        .move_to(
            scene.head,
            Transform::from_translation(Vec3::new(x, 1.0, 0.0)),
        )
        .unwrap();
    if frame == 20 {
        world
            .apply_linear_impulse(
                scene.crate_box,
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.5, 0.3, 0.0),
            )
            .unwrap();
    }
}

fn record(workers: usize) -> (Recording, Scene, u64) {
    let mut world = World::builder().workers(workers).build().unwrap();
    world.start_recording().unwrap();
    let scene = populate(&mut world);
    for frame in 0..60 {
        drive(&mut world, &scene, frame);
        world.advance(1.0 / 60.0);
    }
    let hash = world.state_hash();
    (world.finish_recording().unwrap(), scene, hash)
}

#[test]
fn recording_replays_exactly() {
    let (recording, _, hash) = record(4);
    assert_eq!(recording.ticks(), 120);
    let bytes = recording.to_bytes();
    let decoded = Recording::from_bytes(&bytes).unwrap();
    assert_eq!(decoded.to_bytes(), bytes);
    let mut replayer = decoded.replayer().unwrap();
    while replayer.next_frame().unwrap() {}
    let report = replayer.report();
    assert_eq!(report.divergence, None);
    assert_eq!(report.ticks, 120);
    assert_eq!(replayer.world().state_hash(), hash);
}

#[test]
fn replay_reports_first_divergent_tick() {
    let (recording, scene, _) = record(1);
    let mut replayer = recording.replayer().unwrap();
    let mut frames = 0;
    while replayer.next_frame().unwrap() {
        frames += 1;
        if frames == 30 {
            replayer
                .world()
                .apply_linear_impulse(
                    scene.crate_box,
                    Vec3::new(0.01, 0.0, 0.0),
                    Vec3::new(0.5, 0.1, 0.0),
                )
                .unwrap();
        }
    }
    let divergence = replayer.report().divergence.unwrap();
    assert_eq!(divergence.tick, 61);
    assert_ne!(divergence.expected, divergence.actual);
}

#[test]
fn truncated_recordings_are_rejected() {
    let bytes = record(1).0.to_bytes();
    let result = Recording::from_bytes(&bytes[..bytes.len() - 3]);
    assert!(matches!(result, Err(Error::Recording(_))));
}
