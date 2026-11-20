use rikka_physics_world::{
    Body, Capsule, Cuboid, Plane, Sphere, Spherical, Transform, Vec3, World,
};

fn scene(workers: usize) -> World {
    let mut world = World::builder().workers(workers).build().unwrap();
    world.create_body(Body::fixed().shape(Plane::up())).unwrap();
    let head = world
        .create_body(
            Body::kinematic()
                .position(Vec3::new(0.0, 1.5, 0.0))
                .shape(Sphere::new(0.1)),
        )
        .unwrap();
    for level in 0..4 {
        world
            .create_body(
                Body::dynamic()
                    .position(Vec3::new(0.6, 0.1 + 0.2 * level as f32, 0.0))
                    .shape(Cuboid::new(Vec3::splat(0.1))),
            )
            .unwrap();
    }
    let bob = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 1.5, 0.25))
                .shape(Capsule::new(0.02, 0.06))
                .mass(0.05),
        )
        .unwrap();
    world
        .create_joint(
            Spherical::new(head, bob).frame(Transform::from_translation(Vec3::new(0.0, 1.5, 0.12))),
        )
        .unwrap();
    world
}

fn hashes_at_ticks(world: &mut World, frame_dt: f64, checkpoints: &[i64]) -> Vec<u64> {
    let mut hashes = Vec::new();
    for &checkpoint in checkpoints {
        while world.ticks() < checkpoint {
            world.advance(frame_dt);
        }
        assert_eq!(world.ticks(), checkpoint);
        hashes.push(world.state_hash());
    }
    hashes
}

#[test]
fn results_are_independent_of_frame_rate() {
    let checkpoints = [60, 120, 240];
    let reference = hashes_at_ticks(&mut scene(1), 1.0 / 30.0, &checkpoints);
    for frame_rate in [60.0, 144.0] {
        let hashes = hashes_at_ticks(&mut scene(1), 1.0 / frame_rate, &checkpoints);
        assert_eq!(hashes, reference, "{frame_rate} Hz frames");
    }
}

#[test]
fn results_are_independent_of_worker_count() {
    let checkpoints = [30, 90, 180];
    let serial = hashes_at_ticks(&mut scene(1), 1.0 / 60.0, &checkpoints);
    let parallel = hashes_at_ticks(&mut scene(8), 1.0 / 60.0, &checkpoints);
    assert_eq!(serial, parallel);
}
