use rikka_physics_world::{
    Body, BodyId, Capsule, Plane, Result, Sphere, Spherical, Transform, Vec3, World,
};

const RATE: f64 = 120.0;

fn scene() -> (World, BodyId) {
    let mut world = World::builder().build().unwrap();
    world.create_body(Body::fixed().shape(Plane::up())).unwrap();
    let head = world
        .create_body(
            Body::kinematic()
                .position(Vec3::new(0.0, 1.5, 0.0))
                .shape(Sphere::new(0.1)),
        )
        .unwrap();
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
    (world, head)
}

fn animate(head: BodyId, speed: f64) -> impl FnMut(&mut World, f64) -> Result<()> {
    move |world, time| {
        let offset = Vec3::new((0.3 * (speed * time).sin()) as f32, 0.0, 0.0);
        world.move_to(
            head,
            Transform::from_translation(Vec3::new(0.0, 1.5, 0.0) + offset),
        )
    }
}

fn play(world: &mut World, head: BodyId, ticks: std::ops::RangeInclusive<i64>) {
    let mut drive = animate(head, 3.0);
    for tick in ticks {
        let time = tick as f64 / RATE;
        drive(world, time).unwrap();
        world.advance_to(time);
    }
}

#[test]
fn seek_matches_continuous_run() {
    let (mut straight, head) = scene();
    straight.seek(0.0, 0, animate(head, 3.0)).unwrap();
    play(&mut straight, head, 1..=90);

    let (mut sought, head) = scene();
    sought.seek(0.5, 60, animate(head, 3.0)).unwrap();
    assert_eq!(sought.ticks(), 60);
    play(&mut sought, head, 61..=90);
    assert_eq!(sought.state_hash(), straight.state_hash());
}
