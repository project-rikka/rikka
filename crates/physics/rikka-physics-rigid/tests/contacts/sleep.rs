use rikka_physics_geometry::{Cuboid, Sphere};
use rikka_physics_math::Vec3;
use rikka_physics_rigid::{Body, RigidWorld};

#[test]
fn resting_island_sleeps_and_wakes_on_impact() {
    let mut world = RigidWorld::builder().build().unwrap();
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(0.0, -1.0, 0.0))
                .shape(Cuboid::new(Vec3::new(10.0, 1.0, 10.0))),
        )
        .unwrap();
    let boxes: Vec<_> = (0..3)
        .map(|level| {
            world
                .create_body(
                    Body::dynamic()
                        .position(Vec3::new(0.0, 0.5 + level as f32, 0.0))
                        .shape(Cuboid::new(Vec3::splat(0.5))),
                )
                .unwrap()
        })
        .collect();
    let character = world
        .create_body(
            Body::dynamic()
                .character()
                .position(Vec3::new(4.0, 0.5, 0.0))
                .shape(Cuboid::new(Vec3::splat(0.5))),
        )
        .unwrap();
    let mut asleep_at = None;
    for step in 0..180 {
        world.step(1.0 / 60.0);
        if asleep_at.is_none() && !world.is_awake(boxes[0]).unwrap() {
            asleep_at = Some(step);
            assert!(
                world
                    .body_move_events()
                    .iter()
                    .any(|event| event.fell_asleep)
            );
        }
    }
    let asleep_at = asleep_at.expect("stack never slept");
    assert!(asleep_at >= 25, "slept too early at step {asleep_at}");
    assert!(boxes.iter().all(|&id| !world.is_awake(id).unwrap()));
    assert!(world.is_awake(character).unwrap());

    let ball = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 6.0, 0.0))
                .shape(Sphere::new(0.3)),
        )
        .unwrap();
    let mut woke = false;
    for _ in 0..90 {
        world.step(1.0 / 60.0);
        woke |= world.is_awake(boxes[2]).unwrap();
    }
    assert!(woke, "impact did not wake the stack");
    assert!(
        world.pose(ball).unwrap().p.y > 3.0,
        "ball fell through the stack"
    );

    world
        .apply_linear_impulse(boxes[0], Vec3::X, Vec3::ZERO)
        .unwrap();
    assert!(boxes.iter().all(|&id| world.is_awake(id).unwrap()));
}

#[test]
fn contact_events_begin_and_end() {
    let mut world = RigidWorld::builder().build().unwrap();
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(0.0, -1.0, 0.0))
                .shape(Cuboid::new(Vec3::new(10.0, 1.0, 10.0))),
        )
        .unwrap();
    let ball = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 2.0, 0.0))
                .shape(Sphere::new(0.5)),
        )
        .unwrap();
    let mut begun = 0;
    for _ in 0..90 {
        world.step(1.0 / 60.0);
        begun += world.contact_begin_events().len();
    }
    assert_eq!(begun, 1);
    world
        .set_linear_velocity(ball, Vec3::new(0.0, 20.0, 0.0))
        .unwrap();
    let mut ended = 0;
    for _ in 0..30 {
        world.step(1.0 / 60.0);
        ended += world.contact_end_events().len();
    }
    assert_eq!(ended, 1);
}
