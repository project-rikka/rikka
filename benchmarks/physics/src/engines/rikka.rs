use std::collections::HashMap;
use std::time::Duration;

use glam::Vec3;
use rikka_physics_collision::{Filter, Material};
use rikka_physics_geometry::{ConvexHull, Cuboid, MeshDef, MeshShape, Sphere, TriangleMesh};
use rikka_physics_math::Transform;
use rikka_physics_rigid::{Body, BodyId, Revolute, RigidWorld, ShapeDef, Spherical};

use crate::blueprint::{
    BodyDef, BodyIndex, Form, GRAVITY, JointDef, JointKind, Motion, Pose, SUBSTEPS, Shape,
    TIME_STEP,
};
use crate::error::Result;
use crate::world::{Engine, Settings, World};

#[derive(Debug)]
pub struct Rikka;

impl Engine for Rikka {
    const NAME: &'static str = "rikka";

    fn world(settings: &Settings) -> Result<Box<dyn World>> {
        let world = RigidWorld::builder()
            .gravity(GRAVITY)
            .substeps(SUBSTEPS)
            .workers(settings.workers)
            .sleeping(settings.sleeping)
            .continuous(false)
            .build()?;
        Ok(Box::new(RikkaWorld {
            world,
            sleep_threshold: settings.sleep_threshold,
            bodies: Vec::with_capacity(settings.capacity),
            hulls: HashMap::new(),
            meshes: HashMap::new(),
        }))
    }
}

struct RikkaWorld {
    world: RigidWorld,
    sleep_threshold: f32,
    bodies: Vec<BodyId>,
    hulls: HashMap<u64, ConvexHull>,
    meshes: HashMap<u64, TriangleMesh>,
}

fn transform(pose: Pose) -> Transform {
    Transform::new(pose.position, pose.rotation)
}

fn pose(transform: Transform) -> Pose {
    Pose::new(transform.p, transform.q)
}

impl RikkaWorld {
    fn shape(&mut self, shape: &Shape) -> Result<ShapeDef> {
        let placed = transform(shape.pose);
        let key = shape.geometry.key();
        let geometry = match shape.geometry.form() {
            Form::Cuboid { half } => ShapeDef::new(Cuboid::with_transform(placed, *half)),
            Form::Sphere { radius } => ShapeDef::new(Sphere {
                center: shape.pose.position,
                radius: *radius,
            }),
            Form::Hull(hull) => {
                let built = match self.hulls.get(&key) {
                    Some(built) => built.clone(),
                    None => {
                        let built = ConvexHull::new(&hull.points)?;
                        self.hulls.insert(key, built.clone());
                        built
                    }
                };
                match shape.pose.is_identity() {
                    true => ShapeDef::new(built),
                    false => ShapeDef::new(built.transformed(placed, Vec3::ONE)?),
                }
            }
            Form::Mesh(mesh) => {
                let built = match (self.meshes.get(&key), shape.pose.is_identity()) {
                    (Some(built), true) => built.clone(),
                    (_, identity) => {
                        let vertices: Vec<Vec3> = mesh
                            .vertices
                            .iter()
                            .map(|&vertex| shape.pose.apply(vertex))
                            .collect();
                        let built = TriangleMesh::build(
                            &MeshDef::new(&vertices, &mesh.indices).identify_edges(true),
                        )?
                        .mesh;
                        if identity {
                            self.meshes.insert(key, built.clone());
                        }
                        built
                    }
                };
                ShapeDef::new(MeshShape::new(built))
            }
        };
        Ok(geometry
            .density(shape.density)
            .material(
                Material::default()
                    .friction(shape.friction)
                    .restitution(shape.restitution)
                    .rolling_resistance(shape.rolling_resistance),
            )
            .filter(
                Filter::default()
                    .category(u64::from(shape.category))
                    .mask(u64::from(shape.mask) | !u64::from(u32::MAX)),
            ))
    }
}

impl World for RikkaWorld {
    fn add_body(&mut self, body: &BodyDef) -> Result<BodyIndex> {
        let mut def = match body.motion {
            Motion::Static => Body::fixed(),
            Motion::Kinematic => Body::kinematic(),
            Motion::Dynamic => Body::dynamic(),
        }
        .pose(transform(body.pose))
        .linear_velocity(body.linear_velocity)
        .angular_velocity(body.angular_velocity)
        .sleep(body.sleep)
        .sleep_threshold(self.sleep_threshold);
        for shape in &body.shapes {
            def = def.shape(self.shape(shape)?);
        }
        self.bodies.push(self.world.create_body(def)?);
        Ok(BodyIndex(self.bodies.len() as u32 - 1))
    }

    fn add_joint(&mut self, joint: &JointDef) -> Result<()> {
        let (a, b) = (
            self.bodies[joint.a.0 as usize],
            self.bodies[joint.b.0 as usize],
        );
        let (frame_a, frame_b) = (transform(joint.frame_a), transform(joint.frame_b));
        match joint.kind {
            JointKind::Spherical => {
                self.world
                    .create_joint(Spherical::new(a, b).local_frames(frame_a, frame_b))?;
            }
            JointKind::Revolute(motor) => {
                let mut revolute = Revolute::new(a, b).local_frames(frame_a, frame_b);
                if let Some(motor) = motor {
                    revolute = revolute.motor(motor.speed, motor.max_torque);
                }
                self.world.create_joint(revolute)?;
            }
        }
        Ok(())
    }

    fn prepare(&mut self) {
        self.world.refresh_broadphase();
    }

    fn move_kinematic(&mut self, body: BodyIndex, target: Pose) {
        self.world
            .move_to(self.bodies[body.0 as usize], transform(target))
            .expect("moved body should be kinematic");
    }

    fn step(&mut self) {
        self.world.step(TIME_STEP);
    }

    fn pose(&self, body: BodyIndex) -> Pose {
        pose(
            self.world
                .pose(self.bodies[body.0 as usize])
                .expect("body should exist"),
        )
    }

    fn awake_bodies(&self) -> usize {
        self.world.awake_body_count()
    }

    fn state_hash(&self) -> Option<u64> {
        Some(self.world.state_hash())
    }

    fn enable_profile(&mut self) {
        self.world.profile_mut().set_enabled(true);
    }

    fn profile(&self) -> Vec<(&'static str, Duration, u32)> {
        self.world
            .profile()
            .entries()
            .iter()
            .map(|entry| (entry.name, entry.duration, entry.count))
            .collect()
    }
}
