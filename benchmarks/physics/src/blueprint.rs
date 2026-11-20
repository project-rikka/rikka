use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use glam::{Quat, Vec3};
use rikka_physics_geometry::ConvexHull;

use crate::error::Result;

pub const TIME_STEP: f32 = 1.0 / 60.0;
pub const SUBSTEPS: u32 = 4;
pub const GRAVITY: Vec3 = Vec3::new(0.0, -10.0, 0.0);
const SLEEP_THRESHOLD: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
}

impl Pose {
    pub const IDENTITY: Self = Self {
        position: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };

    pub fn new(position: Vec3, rotation: Quat) -> Self {
        Self { position, rotation }
    }

    pub fn from_position(position: Vec3) -> Self {
        Self::new(position, Quat::IDENTITY)
    }

    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    pub fn apply(&self, point: Vec3) -> Vec3 {
        self.position + self.rotation * point
    }

    pub fn inverse(&self) -> Self {
        let rotation = self.rotation.inverse();
        Self::new(rotation * -self.position, rotation)
    }

    pub fn then(&self, local: Pose) -> Self {
        Self::new(self.apply(local.position), self.rotation * local.rotation)
    }
}

#[derive(Debug)]
pub struct Hull {
    pub points: Vec<Vec3>,
    pub volume: f32,
    pub centroid: Vec3,
}

#[derive(Debug)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u32>,
}

#[derive(Debug)]
pub enum Form {
    Cuboid { half: Vec3 },
    Sphere { radius: f32 },
    Hull(Hull),
    Mesh(Mesh),
}

#[derive(Clone, Debug)]
pub struct Geometry {
    key: u64,
    form: Arc<Form>,
}

impl Geometry {
    fn from_form(form: Form) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Self {
            key: NEXT.fetch_add(1, Ordering::Relaxed),
            form: Arc::new(form),
        }
    }

    pub fn cuboid(half: Vec3) -> Self {
        Self::from_form(Form::Cuboid { half })
    }

    pub fn sphere(radius: f32) -> Self {
        Self::from_form(Form::Sphere { radius })
    }

    pub fn hull(points: Vec<Vec3>) -> Result<Self> {
        let measured = ConvexHull::new(&points)?;
        Ok(Self::from_form(Form::Hull(Hull {
            volume: measured.volume(),
            centroid: measured.center(),
            points,
        })))
    }

    pub fn mesh(vertices: Vec<Vec3>, indices: Vec<u32>) -> Self {
        Self::from_form(Form::Mesh(Mesh { vertices, indices }))
    }

    pub fn form(&self) -> &Form {
        &self.form
    }

    pub fn key(&self) -> u64 {
        self.key
    }

    pub fn volume(&self) -> f32 {
        match self.form() {
            Form::Cuboid { half } => 8.0 * half.x * half.y * half.z,
            Form::Sphere { radius } => 4.0 / 3.0 * std::f32::consts::PI * radius.powi(3),
            Form::Hull(hull) => hull.volume,
            Form::Mesh(_) => 0.0,
        }
    }

    pub fn centroid(&self) -> Vec3 {
        match self.form() {
            Form::Hull(hull) => hull.centroid,
            Form::Cuboid { .. } | Form::Sphere { .. } | Form::Mesh(_) => Vec3::ZERO,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Shape {
    pub geometry: Geometry,
    pub pose: Pose,
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
    pub rolling_resistance: f32,
    pub category: u32,
    pub mask: u32,
}

impl Shape {
    pub fn new(geometry: Geometry) -> Self {
        Self {
            geometry,
            pose: Pose::IDENTITY,
            density: 1.0,
            friction: 0.6,
            restitution: 0.0,
            rolling_resistance: 0.0,
            category: 1,
            mask: u32::MAX,
        }
    }

    #[must_use]
    pub fn placed(mut self, pose: Pose) -> Self {
        self.pose = pose;
        self
    }

    #[must_use]
    pub fn density(mut self, density: f32) -> Self {
        self.density = density;
        self
    }

    #[must_use]
    pub fn friction(mut self, friction: f32) -> Self {
        self.friction = friction;
        self
    }

    #[must_use]
    pub fn restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution;
        self
    }

    #[must_use]
    pub fn rolling_resistance(mut self, rolling_resistance: f32) -> Self {
        self.rolling_resistance = rolling_resistance;
        self
    }

    #[must_use]
    pub fn filter(mut self, category: u32, mask: u32) -> Self {
        self.category = category;
        self.mask = mask;
        self
    }

    fn mass_moment(&self) -> (f32, Vec3) {
        let mass = self.density * self.geometry.volume();
        (mass, mass * self.pose.apply(self.geometry.centroid()))
    }
}

impl From<Geometry> for Shape {
    fn from(geometry: Geometry) -> Self {
        Self::new(geometry)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Static,
    Kinematic,
    Dynamic,
}

#[derive(Clone, Debug)]
pub struct BodyDef {
    pub motion: Motion,
    pub pose: Pose,
    pub linear_velocity: Vec3,
    pub angular_velocity: Vec3,
    pub sleep: bool,
    pub shapes: Vec<Shape>,
}

impl BodyDef {
    fn with_motion(motion: Motion) -> Self {
        Self {
            motion,
            pose: Pose::IDENTITY,
            linear_velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            sleep: true,
            shapes: Vec::new(),
        }
    }

    pub fn fixed() -> Self {
        Self::with_motion(Motion::Static)
    }

    pub fn kinematic() -> Self {
        Self::with_motion(Motion::Kinematic)
    }

    pub fn dynamic() -> Self {
        Self::with_motion(Motion::Dynamic)
    }

    #[must_use]
    pub fn position(mut self, position: Vec3) -> Self {
        self.pose.position = position;
        self
    }

    #[must_use]
    pub fn velocity(mut self, linear: Vec3, angular: Vec3) -> Self {
        self.linear_velocity = linear;
        self.angular_velocity = angular;
        self
    }

    #[must_use]
    pub fn sleep(mut self, sleep: bool) -> Self {
        self.sleep = sleep;
        self
    }

    #[must_use]
    pub fn shape(mut self, shape: impl Into<Shape>) -> Self {
        self.shapes.push(shape.into());
        self
    }

    pub fn center_of_mass(&self) -> Vec3 {
        let (mass, moment) = self
            .shapes
            .iter()
            .map(Shape::mass_moment)
            .fold((0.0, Vec3::ZERO), |(m, c), (mass, moment)| {
                (m + mass, c + moment)
            });
        match mass > 0.0 {
            true => self.pose.apply(moment / mass),
            false => self.pose.position,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BodyIndex(pub u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motor {
    pub speed: f32,
    pub max_torque: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JointKind {
    Spherical,
    Revolute(Option<Motor>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointDef {
    pub kind: JointKind,
    pub a: BodyIndex,
    pub b: BodyIndex,
    pub frame_a: Pose,
    pub frame_b: Pose,
}

#[derive(Debug)]
pub struct Blueprint {
    pub sleeping: bool,
    pub sleep_threshold: f32,
    pub bodies: Vec<BodyDef>,
    pub joints: Vec<JointDef>,
    pub reserve: usize,
}

impl Blueprint {
    pub fn new(sleeping: bool) -> Self {
        Self {
            sleeping,
            sleep_threshold: SLEEP_THRESHOLD,
            bodies: Vec::new(),
            joints: Vec::new(),
            reserve: 0,
        }
    }

    pub fn sleep_below(&mut self, threshold: f32) {
        self.sleep_threshold = threshold;
    }

    pub fn add(&mut self, body: BodyDef) -> BodyIndex {
        self.bodies.push(body);
        BodyIndex(self.bodies.len() as u32 - 1)
    }

    pub fn reserve(&mut self, bodies: usize) {
        self.reserve += bodies;
    }

    pub fn spherical(&mut self, a: BodyIndex, b: BodyIndex, anchor_a: Vec3, anchor_b: Vec3) {
        self.joints.push(JointDef {
            kind: JointKind::Spherical,
            a,
            b,
            frame_a: Pose::from_position(anchor_a),
            frame_b: Pose::from_position(anchor_b),
        });
    }

    pub fn revolute(&mut self, a: BodyIndex, b: BodyIndex, frame: Pose, motor: Option<Motor>) {
        let local = |body: BodyIndex| self.bodies[body.0 as usize].pose.inverse().then(frame);
        let joint = JointDef {
            kind: JointKind::Revolute(motor),
            a,
            b,
            frame_a: local(a),
            frame_b: local(b),
        };
        self.joints.push(joint);
    }

    pub fn capacity(&self) -> usize {
        self.bodies.len() + self.reserve
    }
}
