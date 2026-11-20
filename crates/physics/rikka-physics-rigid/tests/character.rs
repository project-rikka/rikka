use rikka_physics_geometry::Cuboid;
use rikka_physics_math::Vec3;
use rikka_physics_rigid::{Body, Character, RigidWorld};

const DT: f32 = 1.0 / 60.0;
const GRAVITY: f32 = 9.8;

fn ground() -> RigidWorld {
    let mut world = RigidWorld::builder().build().unwrap();
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(0.0, -0.5, 0.0))
                .shape(Cuboid::new(Vec3::new(20.0, 0.5, 20.0))),
        )
        .unwrap();
    world
}

fn walk(world: &mut RigidWorld, character: &mut Character, velocity: Vec3, seconds: f32) {
    let mut vertical = 0.0;
    for _ in 0..(seconds / DT) as u32 {
        world.step(DT);
        vertical = if character.is_grounded() {
            0.0
        } else {
            vertical - GRAVITY * DT
        };
        character.update(world, velocity + Vec3::new(0.0, vertical, 0.0), DT);
    }
}

fn stairs(step_height: f32) -> Vec3 {
    let mut world = ground();
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(3.0, 0.125, 0.0))
                .shape(Cuboid::new(Vec3::new(2.0, 0.125, 2.0))),
        )
        .unwrap();
    let mut character =
        Character::capsule(0.3, 1.8, Vec3::new(0.0, 0.0, 0.0)).step_height(step_height);
    walk(&mut world, &mut character, Vec3::new(2.0, 0.0, 0.0), 2.0);
    character.position()
}

#[test]
fn character_steps_up_only_within_step_height() {
    let stepped = stairs(0.35);
    assert!(
        stepped.x > 2.0 && (stepped.y - 0.25).abs() < 0.05,
        "stepped to {stepped}"
    );
    let blocked = stairs(0.0);
    assert!(blocked.x < 0.8 && blocked.y < 0.05, "walked to {blocked}");
}
