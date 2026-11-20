use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};
use rapier3d::dynamics::{
    CoefficientCombineRule, GenericJoint, RevoluteJointBuilder, RigidBodyBuilder, RigidBodyHandle,
    SphericalJointBuilder,
};
use rapier3d::geometry::{
    ColliderBuilder, Group, InteractionGroups, InteractionTestMode, SharedShape,
};
use rapier3d::math::{Pose as Placement, Rotation, Vector};
use rapier3d::pipeline::PhysicsWorld;
use rapier3d::rayon::{ThreadPool, ThreadPoolBuilder};

use crate::blueprint::{
    BodyDef, BodyIndex, Form, GRAVITY, JointDef, JointKind, Motion, Pose, Shape, TIME_STEP,
};
use crate::error::{Error, Result};
use crate::world::{Engine, Settings, World};

const MOTOR_DAMPING: f32 = 1.0e3;

#[derive(Debug)]
pub struct Rapier;

impl Engine for Rapier {
    const NAME: &'static str = "rapier";

    fn world(settings: &Settings) -> Result<Box<dyn World>> {
        let pool = ThreadPoolBuilder::new()
            .num_threads(settings.workers)
            .thread_name(|worker| format!("rikka-bench-rapier-{worker}"))
            .build()
            .map_err(|_| rejected("thread pool"))?;
        let mut world = PhysicsWorld::new();
        world.gravity = vector(GRAVITY);
        world.integration_parameters.dt = TIME_STEP;
        Ok(Box::new(RapierWorld {
            world,
            pool,
            sleeping: settings.sleeping,
            sleep_threshold: settings.sleep_threshold,
            bodies: Vec::with_capacity(settings.capacity),
            shapes: HashMap::new(),
        }))
    }
}

fn rejected(what: &'static str) -> Error {
    Error::Rejected {
        engine: Rapier::NAME,
        what,
    }
}

fn vector(value: Vec3) -> Vector {
    Vector::from_array(value.to_array())
}

fn placement(pose: Pose) -> Placement {
    Placement::from_parts(
        vector(pose.position),
        Rotation::from_array(pose.rotation.to_array()),
    )
}

fn pose(placed: &Placement) -> Pose {
    Pose::new(
        Vec3::from_array(placed.translation.to_array()),
        Quat::from_array(placed.rotation.to_array()),
    )
}

struct RapierWorld {
    world: PhysicsWorld,
    pool: ThreadPool,
    sleeping: bool,
    sleep_threshold: f32,
    bodies: Vec<RigidBodyHandle>,
    shapes: HashMap<u64, SharedShape>,
}

impl RapierWorld {
    fn geometry(&mut self, shape: &Shape) -> Result<SharedShape> {
        let key = shape.geometry.key();
        if let Some(found) = self.shapes.get(&key) {
            return Ok(found.clone());
        }
        let made = match shape.geometry.form() {
            Form::Cuboid { half } => SharedShape::cuboid(half.x, half.y, half.z),
            Form::Sphere { radius } => SharedShape::ball(*radius),
            Form::Hull(hull) => {
                let points: Vec<Vector> = hull.points.iter().copied().map(vector).collect();
                SharedShape::convex_hull(&points).ok_or_else(|| rejected("convex hull"))?
            }
            Form::Mesh(mesh) => SharedShape::trimesh(
                mesh.vertices.iter().copied().map(vector).collect(),
                mesh.indices.as_chunks::<3>().0.to_vec(),
            )
            .map_err(|_| rejected("triangle mesh"))?,
        };
        self.shapes.insert(key, made.clone());
        Ok(made)
    }

    fn body(&self, body: BodyIndex) -> RigidBodyHandle {
        self.bodies[body.0 as usize]
    }
}

impl World for RapierWorld {
    fn add_body(&mut self, body: &BodyDef) -> Result<BodyIndex> {
        let builder = match body.motion {
            Motion::Static => RigidBodyBuilder::fixed(),
            Motion::Kinematic => RigidBodyBuilder::kinematic_velocity_based(),
            Motion::Dynamic => RigidBodyBuilder::dynamic(),
        }
        .pose(placement(body.pose))
        .linvel(vector(body.linear_velocity))
        .angvel(vector(body.angular_velocity))
        .can_sleep(self.sleeping && body.sleep);
        let handle = self.world.insert_body(builder);
        self.world.bodies[handle]
            .activation_mut()
            .normalized_linear_threshold = self.sleep_threshold;
        for shape in &body.shapes {
            let collider = ColliderBuilder::new(self.geometry(shape)?)
                .position(placement(shape.pose))
                .density(shape.density)
                .friction(shape.friction)
                .restitution(shape.restitution)
                .friction_combine_rule(CoefficientCombineRule::GeometricMean)
                .restitution_combine_rule(CoefficientCombineRule::Max)
                .collision_groups(InteractionGroups::new(
                    Group::from_bits_retain(shape.category),
                    Group::from_bits_retain(shape.mask),
                    InteractionTestMode::And,
                ));
            self.world.insert_collider(collider, Some(handle));
        }
        self.bodies.push(handle);
        Ok(BodyIndex(self.bodies.len() as u32 - 1))
    }

    fn add_joint(&mut self, joint: &JointDef) -> Result<()> {
        let (a, b) = (self.body(joint.a), self.body(joint.b));
        let joint: GenericJoint = match joint.kind {
            JointKind::Spherical => SphericalJointBuilder::new()
                .local_frame1(placement(joint.frame_a))
                .local_frame2(placement(joint.frame_b))
                .into(),
            JointKind::Revolute(motor) => {
                let mut builder = RevoluteJointBuilder::new(Vector::X);
                if let Some(motor) = motor {
                    builder = builder
                        .motor_velocity(motor.speed, MOTOR_DAMPING)
                        .motor_max_force(motor.max_torque);
                }
                let along_z = Quat::from_rotation_y(-FRAC_PI_2);
                let mut generic: GenericJoint = builder.into();
                let frame =
                    |pose: Pose| placement(Pose::new(pose.position, pose.rotation * along_z));
                generic.set_local_frame1(frame(joint.frame_a));
                generic.set_local_frame2(frame(joint.frame_b));
                generic
            }
        };
        self.world.insert_impulse_joint(a, b, joint);
        Ok(())
    }

    fn prepare(&mut self) {}

    fn move_kinematic(&mut self, body: BodyIndex, target: Pose) {
        let handle = self.body(body);
        let rigid = &mut self.world.bodies[handle];
        let current = pose(rigid.position());
        let linear = (target.position - current.position) / TIME_STEP;
        let turn = target.rotation * current.rotation.inverse();
        let (axis, angle) = turn.to_axis_angle();
        let angle = if angle > std::f32::consts::PI {
            angle - std::f32::consts::TAU
        } else {
            angle
        };
        rigid.set_linvel(vector(linear), true);
        rigid.set_angvel(vector(axis * angle / TIME_STEP), true);
    }

    fn step(&mut self) {
        let world = &mut self.world;
        self.pool.install(|| world.step());
    }

    fn pose(&self, body: BodyIndex) -> Pose {
        pose(self.world.bodies[self.body(body)].position())
    }

    fn awake_bodies(&self) -> usize {
        self.world.islands.active_bodies().count()
    }
}
