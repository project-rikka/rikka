use rikka_physics_geometry::Cuboid;
use rikka_physics_math::{Transform, Vec3};
use rikka_physics_rigid::{Body, RigidWorld};

fn ground(world: &mut RigidWorld) {
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(0.0, -1.0, 0.0))
                .shape(Cuboid::new(Vec3::new(20.0, 1.0, 20.0))),
        )
        .unwrap();
}

#[test]
fn box_stack_settles_without_jitter() {
    let mut world = RigidWorld::builder().sleeping(false).build().unwrap();
    ground(&mut world);
    let boxes: Vec<_> = (0..8)
        .map(|level| {
            world
                .create_body(
                    Body::dynamic()
                        .position(Vec3::new(0.0, 0.5 + level as f32 * 1.0, 0.0))
                        .shape(Cuboid::new(Vec3::splat(0.5))),
                )
                .unwrap()
        })
        .collect();
    for _ in 0..240 {
        world.step(1.0 / 60.0);
    }
    let before: Vec<Transform> = boxes.iter().map(|&id| world.pose(id).unwrap()).collect();
    for _ in 0..60 {
        world.step(1.0 / 60.0);
    }
    for (level, (&id, start)) in boxes.iter().zip(&before).enumerate() {
        let pose = world.pose(id).unwrap();
        assert!(
            (pose.p - start.p).length() < 2e-3,
            "box {level} drifted {:?} -> {:?}",
            start.p,
            pose.p
        );
        assert!(
            (pose.p.y - (0.5 + level as f32)).abs() < 0.05,
            "box {level} at {:?}",
            pose.p
        );
        assert!(world.linear_velocity(id).unwrap().length() < 0.02);
    }
}
