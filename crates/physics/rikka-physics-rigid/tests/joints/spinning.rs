use rikka_physics_geometry::Cuboid;
use rikka_physics_math::{Transform, Vec3};
use rikka_physics_rigid::{Axis, Body, D6, Drive, Motion, RigidWorld, Spring};

const REACH: f32 = 0.3;

#[test]
fn sliding_joint_holds_on_fast_spinning_anchor() {
    let spin = Vec3::new(20.0, 55.0, -15.0);
    let offset = Vec3::new(REACH, 0.0, 0.0);
    let mut world = RigidWorld::builder().build().unwrap();
    let anchor = world
        .create_body(Body::kinematic().angular_velocity(spin))
        .unwrap();
    let body = world
        .create_body(
            Body::dynamic()
                .position(offset)
                .angular_velocity(spin)
                .linear_velocity(spin.cross(offset))
                .mass(0.2)
                .shape(Cuboid::new(Vec3::new(REACH, 0.01, 0.01))),
        )
        .unwrap();
    let mut joint = D6::new(anchor, body).frame(Transform::IDENTITY);
    for axis in Axis::LINEAR {
        joint = joint
            .motion(
                axis,
                Motion::Limited {
                    lower: -0.05,
                    upper: 0.05,
                },
            )
            .drive(axis, Drive::spring(Spring::hertz(8.0).damping_ratio(0.2)));
    }
    world.create_joint(joint).unwrap();
    let limit = 2.0 * spin.length() * REACH;
    for step in 0..600 {
        world.step(1.0 / 120.0);
        let speed = world.linear_velocity(body).unwrap().length();
        assert!(speed < limit, "step {step}: the body moves at {speed} m/s");
    }
}

#[derive(Clone, Copy, Debug)]
enum Chain {
    D6,
    Spherical,
}

#[derive(Clone, Copy, Debug)]
enum Parent {
    Velocity,
    Targets,
}

const LINKS: usize = 4;
const LINK: f32 = 0.08;
const HUB: f32 = 0.1;

fn spinning_chain(chain: Chain, drive_mode: Parent, rate: f32, dt: f32, steps: u32) -> f32 {
    let axis = Vec3::new(0.3, 1.0, 0.2).normalize();
    let spin = axis * rate;
    let mut world = RigidWorld::builder().build().unwrap();
    let anchor = match drive_mode {
        Parent::Velocity => world
            .create_body(Body::kinematic().angular_velocity(spin))
            .unwrap(),
        Parent::Targets => world.create_body(Body::kinematic()).unwrap(),
    };
    let radial = axis.cross(Vec3::Z).normalize();
    let mut parent = anchor;
    let mut bodies = Vec::new();
    for link in 0..LINKS {
        let joint_point = radial * (HUB + link as f32 * 2.0 * LINK);
        let center = joint_point + radial * LINK;
        let body = world
            .create_body(
                Body::dynamic()
                    .position(center)
                    .angular_velocity(spin)
                    .linear_velocity(spin.cross(center))
                    .shape(rikka_physics_geometry::Capsule::new(0.02, LINK * 0.8)),
            )
            .unwrap();
        let frame = Transform::from_translation(joint_point);
        match chain {
            Chain::D6 => {
                let mut joint = D6::new(parent, body).frame(frame);
                for axis in Axis::LINEAR {
                    joint = joint.motion(axis, Motion::Locked);
                }
                joint = joint
                    .motion(
                        Axis::Twist,
                        Motion::Limited {
                            lower: -0.3,
                            upper: 0.3,
                        },
                    )
                    .cone(0.6, 0.6);
                for axis in Axis::ANGULAR {
                    joint = joint.drive(
                        axis,
                        rikka_physics_rigid::Drive::spring(Spring::hertz(4.0).damping_ratio(0.5)),
                    );
                }
                world.create_joint(joint).unwrap();
            }
            Chain::Spherical => {
                world
                    .create_joint(rikka_physics_rigid::Spherical::new(parent, body).frame(frame))
                    .unwrap();
            }
        }
        parent = body;
        bodies.push(body);
    }
    let reach = HUB + 2.0 * LINK * LINKS as f32;
    let mut peak = 0.0f32;
    for step in 0..steps {
        if let Parent::Targets = drive_mode {
            let angle = rate * dt * (step + 1) as f32;
            world
                .move_to(
                    anchor,
                    Transform::from_rotation(rikka_physics_math::Quat::from_axis_angle(
                        axis, angle,
                    )),
                )
                .unwrap();
        }
        world.step(dt);
        for &body in &bodies {
            let speed = world.linear_velocity(body).unwrap().length();
            assert!(speed.is_finite(), "step {step}");
            peak = peak.max(speed / (rate * reach));
        }
    }
    peak
}

#[test]
fn chains_stay_bounded_under_fast_parent_rotation() {
    for chain in [Chain::D6, Chain::Spherical] {
        for parent in [Parent::Velocity, Parent::Targets] {
            for rate in [150.0, 300.0] {
                let peak = spinning_chain(chain, parent, rate, 1.0 / 60.0, 240);
                assert!(peak < 5.0, "{chain:?} {parent:?} {rate} rad/s: {peak}");
            }
        }
    }
}
