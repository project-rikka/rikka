use rikka_physics_collision::Filter;
use rikka_physics_geometry::Cuboid;
use rikka_physics_math::{Quat, Transform, Vec3};
use rikka_physics_rigid::{Body, HumanBone, Ragdoll, RagdollBodies, RigidWorld};

const DT: f32 = 1.0 / 60.0;

fn ground(world: &mut RigidWorld) {
    world
        .create_body(
            Body::fixed()
                .position(Vec3::new(0.0, -1.0, 0.0))
                .shape(Cuboid::new(Vec3::new(20.0, 1.0, 20.0))),
        )
        .unwrap();
}

fn sockets(world: &RigidWorld, ragdoll: &RagdollBodies) -> Vec<(HumanBone, HumanBone, Transform)> {
    HumanBone::ALL
        .into_iter()
        .filter_map(|bone| {
            let parent = bone.parent()?;
            let parent_pose = world.pose(ragdoll.body(parent)).unwrap();
            let child_pose = world.pose(ragdoll.body(bone)).unwrap();
            Some((bone, parent, parent_pose.inverse_mul(&child_pose)))
        })
        .collect()
}

#[test]
fn ragdoll_settles_without_exploding() {
    let mut world = RigidWorld::builder().build().unwrap();
    ground(&mut world);
    let ragdoll = Ragdoll::humanoid()
        .joint_friction(0.5)
        .spawn(
            &mut world,
            Transform::new(Vec3::new(0.0, 1.5, 0.0), Quat::from_rotation_x(0.6)),
        )
        .unwrap();
    assert_eq!(ragdoll.bodies().len(), HumanBone::ALL.len());
    assert_eq!(ragdoll.joints().count(), 16);
    let rest = sockets(&world, &ragdoll);
    world
        .set_linear_velocity(ragdoll.body(HumanBone::Chest), Vec3::new(2.0, 0.0, 1.0))
        .unwrap();
    for step in 0..480 {
        world.step(DT);
        for &body in ragdoll.bodies() {
            let pose = world.pose(body).unwrap();
            let speed = world.linear_velocity(body).unwrap().length();
            assert!(pose.p.is_finite() && speed < 20.0, "exploded at {step}");
        }
    }
    for &body in ragdoll.bodies() {
        let pose = world.pose(body).unwrap();
        assert!(
            pose.p.y > -0.05 && pose.p.y < 0.6,
            "not lying down: {pose:?}"
        );
        assert!(world.linear_velocity(body).unwrap().length() < 0.1);
        assert!(world.angular_velocity(body).unwrap().length() < 0.3);
    }
    for ((bone, parent, before), (_, _, after)) in rest.into_iter().zip(sockets(&world, &ragdoll)) {
        assert!(
            (before.p - after.p).length() < 0.02,
            "{bone:?} separated from {parent:?}: {before:?} -> {after:?}"
        );
    }
}

fn pile(workers: usize) -> u64 {
    let mut world = RigidWorld::builder().workers(workers).build().unwrap();
    ground(&mut world);
    for index in 0..24 {
        let angle = index as f32 * 0.9;
        Ragdoll::humanoid()
            .scale(0.9 + 0.01 * index as f32)
            .filter(Filter::default().group(-(index + 1)))
            .spawn(
                &mut world,
                Transform::new(
                    Vec3::new(
                        angle.cos() * 1.5,
                        1.0 + 0.6 * index as f32,
                        angle.sin() * 1.5,
                    ),
                    Quat::from_rotation_y(angle) * Quat::from_rotation_x(1.2),
                ),
            )
            .unwrap();
    }
    for _ in 0..180 {
        world.step(DT);
    }
    world.state_hash()
}

#[test]
fn ragdoll_pile_is_deterministic_across_worker_counts() {
    let single = pile(1);
    assert_eq!(single, pile(4));
    assert_eq!(single, pile(8));
}
