use rikka_physics_geometry::{Cuboid, Sphere};
use rikka_physics_math::{Quat, Transform, Vec3};
use rikka_physics_rigid::{Body, BodyId, RigidWorld, Spherical, Weld};

fn anchor(world: &mut RigidWorld, position: Vec3) -> BodyId {
    world.create_body(Body::fixed().position(position)).unwrap()
}

fn swing(world: &RigidWorld, a: BodyId, b: BodyId, frame_a: Quat, frame_b: Quat) -> f32 {
    let axis_a = world.pose(a).unwrap().q * frame_a * Vec3::Z;
    let axis_b = world.pose(b).unwrap().q * frame_b * Vec3::Z;
    axis_a.dot(axis_b).clamp(-1.0, 1.0).acos()
}

#[test]
fn spherical_chain_stays_connected() {
    let mut world = RigidWorld::builder().build().unwrap();
    let mut previous = anchor(&mut world, Vec3::new(0.0, 10.0, 0.0));
    let mut links = Vec::new();
    for link in 0..12 {
        let body = world
            .create_body(
                Body::dynamic()
                    .position(Vec3::new(0.5 * (link + 1) as f32, 10.0, 0.0))
                    .shape(Sphere::new(0.15)),
            )
            .unwrap();
        let point = Vec3::new(0.5 * link as f32 + 0.25, 10.0, 0.0);
        world
            .create_joint(Spherical::new(previous, body).frame(Transform::from_translation(point)))
            .unwrap();
        links.push((previous, body, point));
        previous = body;
    }
    for _ in 0..240 {
        world.step(1.0 / 60.0);
    }
    for (index, (a, b, point)) in links.into_iter().enumerate() {
        let start_a = if index == 0 {
            Vec3::new(0.0, 10.0, 0.0)
        } else {
            Vec3::new(0.5 * index as f32, 10.0, 0.0)
        };
        let start_b = Vec3::new(0.5 * (index + 1) as f32, 10.0, 0.0);
        let anchor_a = world.pose(a).unwrap().apply(point - start_a);
        let anchor_b = world.pose(b).unwrap().apply(point - start_b);
        assert!(
            (anchor_a - anchor_b).length() < 0.01,
            "joint {index} opened by {}",
            (anchor_a - anchor_b).length()
        );
    }
    let last = world.pose(previous).unwrap().p;
    assert!(last.y < 9.0, "chain did not fall: {last:?}");
}

#[test]
fn spherical_cone_and_twist_limits_hold() {
    let mut world = RigidWorld::builder().build().unwrap();
    let hinge = Vec3::new(0.0, 5.0, 0.0);
    let base = anchor(&mut world, hinge);
    let frame = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let body = world
        .create_body(
            Body::dynamic()
                .position(hinge + Vec3::new(1.0, 0.0, 0.0))
                .angular_velocity(Vec3::new(3.0, 0.0, 0.0))
                .shape(Cuboid::new(Vec3::new(0.5, 0.1, 0.1))),
        )
        .unwrap();
    let cone = 0.4;
    world
        .create_joint(
            Spherical::new(base, body)
                .frame(Transform::new(hinge, frame))
                .cone(cone)
                .twist(-0.2..=0.2),
        )
        .unwrap();
    let frame_b = world.pose(body).unwrap().q.inverse() * frame;
    for step in 0..300 {
        world.step(1.0 / 60.0);
        if step > 120 {
            assert!(
                swing(&world, base, body, frame, frame_b) < cone + 0.05,
                "cone exceeded at step {step}"
            );
        }
    }
    let relative = frame.inverse() * world.pose(body).unwrap().q * frame_b;
    let twist = 2.0 * relative.z.atan2(relative.w);
    let twist = if twist > std::f32::consts::PI {
        twist - std::f32::consts::TAU
    } else {
        twist
    };
    assert!(twist.abs() < 0.25, "twist {twist}");
    let offset = world.pose(body).unwrap().apply(Vec3::new(-1.0, 0.0, 0.0)) - hinge;
    assert!(offset.length() < 0.02, "anchor drifted {offset:?}");
}

#[test]
fn weld_holds_relative_pose_under_load() {
    let mut world = RigidWorld::builder().build().unwrap();
    let base = anchor(&mut world, Vec3::new(0.0, 3.0, 0.0));
    let arm = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(1.0, 3.0, 0.0))
                .shape(Cuboid::new(Vec3::new(1.0, 0.1, 0.1))),
        )
        .unwrap();
    let tip = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(2.5, 3.0, 0.0))
                .shape(Cuboid::new(Vec3::new(0.5, 0.1, 0.1))),
        )
        .unwrap();
    world
        .create_joint(
            Weld::new(base, arm).frame(Transform::from_translation(Vec3::new(0.0, 3.0, 0.0))),
        )
        .unwrap();
    world
        .create_joint(
            Weld::new(arm, tip).frame(Transform::from_translation(Vec3::new(2.0, 3.0, 0.0))),
        )
        .unwrap();
    for _ in 0..180 {
        world.step(1.0 / 60.0);
    }
    let arm_pose = world.pose(arm).unwrap();
    let tip_pose = world.pose(tip).unwrap();
    assert!(
        (arm_pose.p - Vec3::new(1.0, 3.0, 0.0)).length() < 0.05,
        "arm {arm_pose:?}"
    );
    assert!(arm_pose.q.angle_between(Quat::IDENTITY) < 0.05);
    let relative = arm_pose.inverse_mul(&tip_pose);
    assert!(
        (relative.p - Vec3::new(1.5, 0.0, 0.0)).length() < 0.02,
        "tip {relative:?}"
    );
    assert!(relative.q.angle_between(Quat::IDENTITY) < 0.03);
}
