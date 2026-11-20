use std::f32::consts::TAU;

use rikka_physics_geometry::{Cuboid, Sphere};
use rikka_physics_math::{Level, Quat, Simd, Transform, Vec3};
use rikka_physics_rigid::{
    Axis, Body, BodyId, D6, Distance, Drive, Joint, JointDef, JointEventKind, JointId, Motion,
    Revolute, RigidWorld, ShapeDef, Spring,
};

const DT: f32 = 1.0 / 60.0;
const ANGLE_SLOP: f32 = 0.03;
const LINEAR_SLOP: f32 = 0.01;

struct Rig {
    world: RigidWorld,
    base: BodyId,
    body: BodyId,
    local_a: Transform,
    local_b: Transform,
    joint: JointId,
}

impl Rig {
    fn new<T>(
        gravity: Vec3,
        body: Body,
        frame: Transform,
        joint: impl FnOnce(BodyId, BodyId) -> Joint<T>,
    ) -> Self
    where
        Joint<T>: Into<JointDef>,
    {
        let mut world = RigidWorld::builder().gravity(gravity).build().unwrap();
        let base = world.create_body(Body::fixed().position(frame.p)).unwrap();
        let body = world.create_body(body).unwrap();
        let local_a = world.pose(base).unwrap().inverse_mul(&frame);
        let local_b = world.pose(body).unwrap().inverse_mul(&frame);
        let joint = world.create_joint(joint(base, body).frame(frame)).unwrap();
        Self {
            world,
            base,
            body,
            local_a,
            local_b,
            joint,
        }
    }

    fn relative(&self) -> Transform {
        let a = self.world.pose(self.base).unwrap().mul(&self.local_a);
        let b = self.world.pose(self.body).unwrap().mul(&self.local_b);
        a.inverse_mul(&b)
    }

    fn stress(
        &mut self,
        steps: usize,
        torque: f32,
        force: f32,
        mut check: impl FnMut(&Self, usize),
    ) {
        for step in 0..steps {
            let t = step as f32 * DT;
            let direction = Vec3::new((1.3 * t).sin(), (0.7 * t).cos(), (2.1 * t + 1.0).sin());
            let center = self.world.center_of_mass(self.body).unwrap();
            self.world
                .apply_torque(self.body, direction * torque)
                .unwrap();
            self.world
                .apply_force(
                    self.body,
                    Vec3::new(direction.z, direction.y, direction.x) * force,
                    center,
                )
                .unwrap();
            self.world.step(DT);
            if step >= 30 {
                check(self, step);
            }
        }
    }
}

fn swing_twist(q: Quat) -> Vec3 {
    let q = if q.w < 0.0 { -q } else { q };
    let d = q.z * q.z + q.w * q.w;
    Vec3::new(
        2.0 * (q.w * q.x - q.y * q.z).atan2(d),
        2.0 * (q.w * q.y + q.x * q.z).atan2(d),
        2.0 * q.z.atan2(q.w),
    )
}

fn in_range(value: f32, lower: f32, upper: f32, slop: f32) -> bool {
    value >= lower - slop && value <= upper + slop
}

fn arm() -> Body {
    Body::dynamic()
        .position(Vec3::new(0.0, 0.0, 0.6))
        .shape(Cuboid::new(Vec3::new(0.1, 0.1, 0.5)))
}

#[test]
fn d6_pyramid_and_twist_limits_hold_under_stress() {
    let mut rig = Rig::new(
        Vec3::new(0.0, -9.81, 0.0),
        arm(),
        Transform::IDENTITY,
        |a, b| {
            D6::new(a, b)
                .motion(
                    Axis::Twist,
                    Motion::Limited {
                        lower: -0.3,
                        upper: 0.5,
                    },
                )
                .motion(
                    Axis::SwingX,
                    Motion::Limited {
                        lower: -0.2,
                        upper: 0.4,
                    },
                )
                .motion(
                    Axis::SwingY,
                    Motion::Limited {
                        lower: -0.6,
                        upper: 0.1,
                    },
                )
        },
    );
    rig.stress(600, 40.0, 0.0, |rig, step| {
        let angles = swing_twist(rig.relative().q);
        assert!(
            in_range(angles.x, -0.2, 0.4, ANGLE_SLOP),
            "swing x {angles} at {step}"
        );
        assert!(
            in_range(angles.y, -0.6, 0.1, ANGLE_SLOP),
            "swing y {angles} at {step}"
        );
        assert!(
            in_range(angles.z, -0.3, 0.5, ANGLE_SLOP),
            "twist {angles} at {step}"
        );
        assert!(rig.relative().p.length() < LINEAR_SLOP, "anchor {step}");
    });
}

#[test]
fn d6_linear_limits_hold_under_stress() {
    let mut rig = Rig::new(
        Vec3::new(0.0, -9.81, 0.0),
        arm(),
        Transform::IDENTITY,
        |a, b| {
            D6::new(a, b)
                .motion(
                    Axis::LinearX,
                    Motion::Limited {
                        lower: -0.2,
                        upper: 0.3,
                    },
                )
                .motion(
                    Axis::LinearY,
                    Motion::Limited {
                        lower: -0.1,
                        upper: 0.05,
                    },
                )
        },
    );
    rig.stress(600, 5.0, 400.0, |rig, step| {
        let p = rig.relative().p;
        assert!(in_range(p.x, -0.2, 0.3, LINEAR_SLOP), "x {p} at {step}");
        assert!(in_range(p.y, -0.1, 0.05, LINEAR_SLOP), "y {p} at {step}");
        assert!(p.z.abs() < LINEAR_SLOP, "z {p} at {step}");
        assert!(swing_twist(rig.relative().q).length() < ANGLE_SLOP);
    });
}

#[test]
fn distance_range_holds_under_stress() {
    let mut world = RigidWorld::builder().build().unwrap();
    let base = world.create_body(Body::fixed()).unwrap();
    let ball = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(1.5, 0.0, 0.0))
                .shape(Sphere::new(0.1)),
        )
        .unwrap();
    world
        .create_joint(Distance::new(base, ball).range(1.0..=2.0))
        .unwrap();
    let mut longest = 0.0f32;
    for step in 0..600 {
        if step % 90 == 0 {
            let push = Vec3::new((step as f32).sin(), 1.0, (step as f32).cos()) * 12.0;
            world.set_linear_velocity(ball, push).unwrap();
        }
        world.step(DT);
        let length = world.pose(ball).unwrap().p.length();
        longest = longest.max(length);
        assert!(
            in_range(length, 1.0, 2.0, LINEAR_SLOP),
            "length {length} at {step}"
        );
    }
    assert!(
        longest > 1.95,
        "the ball never reached the range end: {longest}"
    );
}

fn oscillation_hertz(mut sample: impl FnMut() -> f32, steps: usize) -> f32 {
    let mut previous = sample();
    let mut crossings = Vec::new();
    for step in 1..=steps {
        let value = sample();
        if previous < 0.0 && value >= 0.0 {
            let fraction = previous / (previous - value);
            crossings.push((step as f32 - 1.0 + fraction) * DT);
        }
        previous = value;
    }
    assert!(crossings.len() >= 3, "no oscillation");
    let periods = crossings.len() - 1;
    periods as f32 / (crossings[periods] - crossings[0])
}

#[test]
fn linear_spring_oscillates_at_its_frequency() {
    for (spring, expected) in [
        (Spring::hertz(1.5).damping_ratio(0.0), 1.5),
        (Spring::stiffness(80.0), (80.0f32 / 2.0).sqrt() / TAU),
    ] {
        let body = Body::dynamic().mass(2.0).shape(Sphere::new(0.2));
        let mut rig = Rig::new(Vec3::ZERO, body, Transform::IDENTITY, |a, b| {
            D6::new(a, b)
                .motion(Axis::LinearY, Motion::Free)
                .drive(Axis::LinearY, Drive::spring(spring))
                .target(Transform::from_translation(Vec3::new(0.0, 0.2, 0.0)))
        });
        let hertz = oscillation_hertz(
            || {
                rig.world.step(DT);
                rig.relative().p.y - 0.2
            },
            600,
        );
        assert!(
            (hertz - expected).abs() < 0.03 * expected,
            "{spring:?}: {hertz} Hz, expected {expected}"
        );
    }
}

#[test]
fn revolute_motor_respects_torque_cap() {
    let (speed, torque) = (5.0, 2.0);
    let mut rig = Rig::new(Vec3::ZERO, arm(), Transform::IDENTITY, |a, b| {
        Revolute::new(a, b).motor(speed, torque)
    });
    let mut previous = 0.0;
    let mut max_acceleration = 0.0f32;
    for _ in 0..600 {
        rig.world.step(DT);
        let omega = rig.world.angular_velocity(rig.body).unwrap().z;
        max_acceleration = max_acceleration.max((omega - previous) / DT);
        previous = omega;
        let (_, reaction) = rig.world.joint_reaction(rig.joint).unwrap();
        assert!(reaction <= torque * 1.02, "torque {reaction} over the cap");
    }
    let mass = rig.world.mass(rig.body).unwrap();
    let inertia_z = mass * (0.2f32 * 0.2 + 0.2 * 0.2) / 12.0;
    assert!(
        max_acceleration <= torque / inertia_z * 1.05,
        "acceleration {max_acceleration} exceeds {}",
        torque / inertia_z
    );
    assert!((previous - speed).abs() < 0.01 * speed, "speed {previous}");
}

#[test]
fn pose_drive_tracks_target_pose() {
    let mut world = RigidWorld::builder().build().unwrap();
    let anchor = world
        .create_body(Body::kinematic().position(Vec3::new(0.0, 2.0, 0.0)))
        .unwrap();
    let body = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 2.0, 0.0))
                .shape(Cuboid::new(Vec3::splat(0.2))),
        )
        .unwrap();
    let joint = world
        .create_joint(
            D6::new(anchor, body)
                .linear(Motion::Free)
                .angular(Motion::Free)
                .pose_drive(
                    Drive::spring(Spring::hertz(8.0)),
                    Drive::spring(Spring::hertz(8.0)),
                ),
        )
        .unwrap();
    let target = Transform::new(Vec3::new(0.3, -0.2, 0.1), Quat::from_rotation_y(0.7));
    world.joint_mut::<D6>(joint).unwrap().set_target(target);
    for _ in 0..240 {
        world.step(DT);
    }
    let anchor_pose = world.pose(anchor).unwrap();
    let relative = anchor_pose.inverse_mul(&world.pose(body).unwrap());
    assert!((relative.p - target.p).length() < 0.02, "{relative:?}");
    assert!(relative.q.angle_between(target.q) < 0.02, "{relative:?}");
}

#[test]
fn overloaded_joints_break_and_report() {
    let mut world = RigidWorld::builder().build().unwrap();
    let base = world.create_body(Body::fixed()).unwrap();
    let weight = |world: &mut RigidWorld, x: f32| {
        world
            .create_body(
                Body::dynamic()
                    .position(Vec3::new(x, -1.0, 0.0))
                    .mass(50.0)
                    .shape(Sphere::new(0.2)),
            )
            .unwrap()
    };
    let d6_weight = weight(&mut world, 0.0);
    let hinge_weight = weight(&mut world, 3.0);
    let rope_weight = weight(&mut world, 6.0);
    let intact_weight = weight(&mut world, 9.0);
    let joints = [
        world
            .create_joint(
                D6::new(base, d6_weight)
                    .frame(Transform::from_translation(Vec3::ZERO))
                    .angular(Motion::Free)
                    .break_force(200.0),
            )
            .unwrap(),
        world
            .create_joint(
                Revolute::new(base, hinge_weight)
                    .frame(Transform::from_translation(Vec3::new(3.0, -1.0, 0.0)))
                    .break_torque(10.0),
            )
            .unwrap(),
        world
            .create_joint(
                Distance::new(base, rope_weight)
                    .local_frames(
                        Transform::from_translation(Vec3::new(6.0, 0.0, 0.0)),
                        Transform::IDENTITY,
                    )
                    .break_force(300.0),
            )
            .unwrap(),
    ];
    let intact = world
        .create_joint(
            D6::new(base, intact_weight)
                .frame(Transform::from_translation(Vec3::new(9.0, 0.0, 0.0)))
                .angular(Motion::Free)
                .break_force(5000.0),
        )
        .unwrap();
    let mut broken = Vec::new();
    for _ in 0..30 {
        if !world.is_joint_broken(joints[1]).unwrap() {
            world
                .apply_torque(hinge_weight, Vec3::new(200.0, 0.0, 0.0))
                .unwrap();
        }
        world.step(DT);
        broken.extend(
            world
                .joint_events()
                .iter()
                .filter(|event| event.kind == JointEventKind::Broken)
                .map(|event| event.joint),
        );
    }
    for joint in joints {
        assert!(broken.contains(&joint), "{joint:?} did not report breaking");
        assert!(world.is_joint_broken(joint).unwrap());
    }
    assert!(!world.is_joint_broken(intact).unwrap());
    assert!(world.pose(d6_weight).unwrap().p.y < -1.5);
}

#[test]
fn bullet_6dof_params_map_to_d6() {
    let position_min = [-0.1, 1.0, 0.0];
    let position_max = [0.1, 0.0, 0.0];
    let rotation_min = [-0.3, -0.2, -0.4];
    let rotation_max = [0.3, 0.2, 0.4];
    let spring_rotation = [30.0, 30.0, 0.0];
    let mut world = RigidWorld::builder().build().unwrap();
    let base = world
        .create_body(Body::fixed().position(Vec3::new(0.0, 5.0, 0.0)))
        .unwrap();
    let body = world
        .create_body(
            Body::dynamic()
                .position(Vec3::new(0.0, 5.0, 0.4))
                .mass(1.0)
                .shape(Cuboid::new(Vec3::new(0.05, 0.05, 0.4))),
        )
        .unwrap();
    let frame = Transform::from_translation(Vec3::new(0.0, 5.0, 0.0));
    let mut joint = D6::new(base, body).frame(frame);
    for (index, axis) in Axis::LINEAR.into_iter().enumerate() {
        joint = joint.motion(
            axis,
            Motion::from_bounds(position_min[index], position_max[index]),
        );
    }
    for (index, axis) in Axis::ANGULAR.into_iter().enumerate() {
        joint = joint.motion(
            axis,
            Motion::from_bounds(rotation_min[index], rotation_max[index]),
        );
        if spring_rotation[index] > 0.0 {
            joint = joint.drive(
                axis,
                Drive::spring(Spring::stiffness(spring_rotation[index])),
            );
        }
    }
    world.create_joint(joint).unwrap();
    let local_b = world.pose(body).unwrap().inverse_mul(&frame);
    for step in 0..300 {
        if step % 60 == 0 {
            world
                .set_angular_velocity(body, Vec3::new(4.0, -3.0, 5.0))
                .unwrap();
        }
        world.step(DT);
        let relative = frame.inverse_mul(&world.pose(body).unwrap().mul(&local_b));
        if step % 60 < 10 {
            continue;
        }
        let angles = swing_twist(relative.q);
        for index in 0..3 {
            assert!(
                in_range(
                    angles[index],
                    rotation_min[index],
                    rotation_max[index],
                    ANGLE_SLOP
                ),
                "axis {index}: {angles} at {step}"
            );
        }
        assert!(
            in_range(relative.p.x, -0.1, 0.1, LINEAR_SLOP),
            "{relative:?}"
        );
        assert!(relative.p.z.abs() < LINEAR_SLOP, "{relative:?}");
    }
    let relative = frame.inverse_mul(&world.pose(body).unwrap().mul(&local_b));
    assert!(
        relative.p.y < -2.0,
        "the free axis follows gravity: {relative:?}"
    );
}

#[test]
fn motor_spins_paddle_on_every_simd_level() {
    for simd in Level::supported().map(Simd::new) {
        let mut world = RigidWorld::builder().simd(simd).build().unwrap();
        let ground = world
            .create_body(Body::fixed().shape(ShapeDef::new(Cuboid::new(Vec3::new(4.0, 0.1, 4.0)))))
            .unwrap();
        let pivot = Vec3::new(0.0, 1.5, 0.0);
        let paddle = world
            .create_body(
                Body::dynamic()
                    .position(pivot)
                    .sleep(false)
                    .shape(ShapeDef::new(Cuboid::new(Vec3::new(0.1, 1.2, 0.5)))),
            )
            .unwrap();
        world
            .create_joint(
                Revolute::new(ground, paddle)
                    .frame(Transform::from_translation(pivot))
                    .motor(4.0, f32::MAX),
            )
            .unwrap();
        for index in 0..12 {
            let x = index as f32 * 0.3 - 1.8;
            world
                .create_body(
                    Body::dynamic()
                        .position(Vec3::new(x, 0.4, 0.0))
                        .shape(ShapeDef::new(Sphere::new(0.15))),
                )
                .unwrap();
        }
        for _ in 0..120 {
            world.step(DT);
        }
        let spin = world.angular_velocity(paddle).unwrap().z;
        assert!((spin - 4.0).abs() < 0.05, "{} spin {spin}", simd.name());
    }
}
