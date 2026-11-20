use std::f32::consts::{FRAC_PI_2, PI};

use rikka_physics_collision::Filter;
use rikka_physics_geometry::Capsule;
use rikka_physics_math::{Quat, Transform, Vec3};

use super::bodies::RagdollBodies;
use super::bone::{HumanBone, Socket};
use crate::joint::d6::D6;
use crate::joint::drive::Drive;
use crate::joint::filter::FilterJoint;
use crate::joint::motion::{Axis, Motion};
use crate::joint::spring::Spring;
use crate::{Body, Result, RigidWorld, ShapeDef};

const FILTERED: [(HumanBone, HumanBone); 5] = [
    (HumanBone::ThighLeft, HumanBone::ThighRight),
    (HumanBone::ThighLeft, HumanBone::Spine),
    (HumanBone::ThighRight, HumanBone::Spine),
    (HumanBone::UpperArmLeft, HumanBone::Spine),
    (HumanBone::UpperArmRight, HumanBone::Spine),
];

#[derive(Clone, Copy, Debug, PartialEq)]
struct Segment {
    head: Transform,
    length: f32,
    radius: f32,
}

#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ragdoll {
    segments: [Segment; 12],
    density: f32,
    friction_torque: f32,
    spring: Option<Spring>,
    filter: Filter,
}

impl Ragdoll {
    pub fn humanoid() -> Self {
        let upright = |x: f32, y: f32, length: f32, radius: f32| Segment {
            head: Transform::from_translation(Vec3::new(x, y, 0.0)),
            length,
            radius,
        };
        let hanging = |x: f32, y: f32, length: f32, radius: f32| Segment {
            head: Transform::new(Vec3::new(x, y, 0.0), Quat::from_rotation_z(PI)),
            length,
            radius,
        };
        let arm = |side: f32, head: Vec3, length: f32, radius: f32| Segment {
            head: Transform::new(
                head,
                Quat::from_rotation_arc(Vec3::Y, Vec3::new(0.25 * side, -1.0, 0.0).normalize()),
            ),
            length,
            radius,
        };
        let shoulder = |side: f32| Vec3::new(0.19 * side, 1.43, 0.0);
        let elbow =
            |side: f32| shoulder(side) + 0.28 * Vec3::new(0.25 * side, -1.0, 0.0).normalize();
        let mut segments = [upright(0.0, 0.0, 0.0, 0.0); 12];
        let mut set = |bone: HumanBone, segment: Segment| segments[bone.index()] = segment;
        set(HumanBone::Pelvis, upright(0.0, 0.93, 0.1, 0.13));
        set(HumanBone::Spine, upright(0.0, 1.05, 0.18, 0.12));
        set(HumanBone::Chest, upright(0.0, 1.25, 0.16, 0.15));
        set(HumanBone::Head, upright(0.0, 1.5, 0.2, 0.1));
        set(HumanBone::UpperArmLeft, arm(1.0, shoulder(1.0), 0.28, 0.05));
        set(HumanBone::LowerArmLeft, arm(1.0, elbow(1.0), 0.27, 0.045));
        set(
            HumanBone::UpperArmRight,
            arm(-1.0, shoulder(-1.0), 0.28, 0.05),
        );
        set(
            HumanBone::LowerArmRight,
            arm(-1.0, elbow(-1.0), 0.27, 0.045),
        );
        set(HumanBone::ThighLeft, hanging(0.09, 0.93, 0.43, 0.08));
        set(HumanBone::CalfLeft, hanging(0.09, 0.5, 0.42, 0.06));
        set(HumanBone::ThighRight, hanging(-0.09, 0.93, 0.43, 0.08));
        set(HumanBone::CalfRight, hanging(-0.09, 0.5, 0.42, 0.06));
        Self {
            segments,
            density: 1000.0,
            friction_torque: 0.05,
            spring: None,
            filter: Filter::default(),
        }
    }

    pub fn bone(mut self, bone: HumanBone, head: Transform, length: f32, radius: f32) -> Self {
        self.segments[bone.index()] = Segment {
            head,
            length,
            radius,
        };
        self
    }

    pub fn scale(mut self, factor: f32) -> Self {
        for segment in &mut self.segments {
            segment.head.p *= factor;
            segment.length *= factor;
            segment.radius *= factor;
        }
        self
    }

    pub fn density(mut self, density: f32) -> Self {
        self.density = density;
        self
    }

    pub fn joint_friction(mut self, torque: f32) -> Self {
        self.friction_torque = torque;
        self
    }

    pub fn joint_spring(mut self, spring: Spring) -> Self {
        self.spring = Some(spring);
        self
    }

    pub fn filter(mut self, filter: Filter) -> Self {
        self.filter = filter;
        self
    }

    pub fn spawn(&self, world: &mut RigidWorld, pose: Transform) -> Result<RagdollBodies> {
        let mut bodies = Vec::with_capacity(HumanBone::ALL.len());
        for bone in HumanBone::ALL {
            let segment = self.segments[bone.index()];
            let capsule = Capsule::segment(
                Vec3::ZERO,
                Vec3::new(0.0, segment.length, 0.0),
                segment.radius,
            );
            bodies.push(
                world.create_body(
                    Body::dynamic().pose(pose.mul(&segment.head)).shape(
                        ShapeDef::new(capsule)
                            .density(self.density)
                            .filter(self.filter),
                    ),
                )?,
            );
        }
        let mut joints = vec![None; HumanBone::ALL.len()];
        for bone in HumanBone::ALL {
            let (Some(parent), Some(socket)) = (bone.parent(), bone.socket()) else {
                continue;
            };
            let head = pose.mul(&self.segments[bone.index()].head);
            let joint = D6::new(bodies[parent.index()], bodies[bone.index()]);
            let joint = match socket {
                Socket::Ball {
                    cone,
                    twist,
                    friction,
                } => {
                    let friction = friction * self.friction_torque;
                    let joint = joint
                        .frame(
                            head.mul(&Transform::from_rotation(Quat::from_rotation_x(-FRAC_PI_2))),
                        )
                        .cone(cone, cone)
                        .motion(
                            Axis::Twist,
                            Motion::Limited {
                                lower: -twist,
                                upper: twist,
                            },
                        )
                        .motor(Axis::SwingX, friction)
                        .motor(Axis::SwingY, friction)
                        .motor(Axis::Twist, friction);
                    match self.spring {
                        Some(spring) => joint.angular_pose_drive(Drive::spring(spring)),
                        None => joint,
                    }
                }
                Socket::Hinge { limit, friction } => {
                    let joint = joint
                        .frame(
                            head.mul(&Transform::from_rotation(Quat::from_rotation_y(FRAC_PI_2))),
                        )
                        .motion(
                            Axis::Twist,
                            Motion::Limited {
                                lower: -limit,
                                upper: limit,
                            },
                        )
                        .motor(Axis::Twist, friction * self.friction_torque);
                    match self.spring {
                        Some(spring) => joint.drive(Axis::Twist, Drive::spring(spring)),
                        None => joint,
                    }
                }
            };
            joints[bone.index()] = Some(world.create_joint(joint)?);
        }
        let mut filters = Vec::with_capacity(FILTERED.len());
        for (first, second) in FILTERED {
            filters.push(world.create_joint(FilterJoint::new(
                bodies[first.index()],
                bodies[second.index()],
            ))?);
        }
        Ok(RagdollBodies {
            bodies,
            joints,
            filters,
        })
    }
}
