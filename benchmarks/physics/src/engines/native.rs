use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;

use glam::{Quat, Vec3};

use crate::blueprint::{
    BodyDef, BodyIndex, Form, GRAVITY, JointDef, JointKind, Motion, Pose, SUBSTEPS, Shape,
    TIME_STEP,
};
use crate::error::{Error, Result};
use crate::world::{Engine, Settings, World};

#[repr(C)]
#[derive(Clone, Copy)]
struct RawVec3 {
    x: f32,
    y: f32,
    z: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawQuat {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RawPose {
    p: RawVec3,
    q: RawQuat,
}

#[repr(C)]
struct RawShape {
    geometry: u32,
    key: u64,
    pose: RawPose,
    half_extents: RawVec3,
    radius: f32,
    points: *const RawVec3,
    point_count: u32,
    indices: *const u32,
    index_count: u32,
    volume: f32,
    centroid: RawVec3,
    density: f32,
    friction: f32,
    restitution: f32,
    rolling_resistance: f32,
    category: u32,
    mask: u32,
}

#[repr(C)]
struct RawBody {
    motion: u32,
    pose: RawPose,
    linear_velocity: RawVec3,
    angular_velocity: RawVec3,
    sleep: bool,
    shapes: *const RawShape,
    shape_count: u32,
}

#[repr(C)]
struct RawJoint {
    kind: u32,
    body_a: u32,
    body_b: u32,
    frame_a: RawPose,
    frame_b: RawPose,
    motor: bool,
    motor_speed: f32,
    max_motor_torque: f32,
}

#[repr(C)]
struct RawWorldDef {
    workers: u32,
    substeps: u32,
    sleeping: bool,
    sleep_threshold: f32,
    gravity: RawVec3,
    max_bodies: u32,
}

#[repr(C)]
struct RawEngine {
    create: unsafe extern "C" fn(*const RawWorldDef) -> *mut c_void,
    destroy: unsafe extern "C" fn(*mut c_void),
    add_body: unsafe extern "C" fn(*mut c_void, *const RawBody) -> u32,
    add_joint: unsafe extern "C" fn(*mut c_void, *const RawJoint) -> bool,
    prepare: unsafe extern "C" fn(*mut c_void),
    move_kinematic: unsafe extern "C" fn(*mut c_void, u32, RawPose, f32),
    step: unsafe extern "C" fn(*mut c_void, f32),
    pose: unsafe extern "C" fn(*const c_void, u32) -> RawPose,
    awake_bodies: unsafe extern "C" fn(*const c_void) -> u32,
}

unsafe extern "C" {
    safe fn box3d_bench_engine() -> &'static RawEngine;
    safe fn jolt_bench_engine() -> &'static RawEngine;
    safe fn bullet_bench_engine() -> &'static RawEngine;
}

const INVALID_BODY: u32 = u32::MAX;

const CUBOID: u32 = 0;
const SPHERE: u32 = 1;
const HULL: u32 = 2;
const MESH: u32 = 3;

const SPHERICAL: u32 = 0;
const REVOLUTE: u32 = 1;

fn vec3(v: Vec3) -> RawVec3 {
    RawVec3 {
        x: v.x,
        y: v.y,
        z: v.z,
    }
}

fn raw_pose(pose: Pose) -> RawPose {
    let q = pose.rotation;
    RawPose {
        p: vec3(pose.position),
        q: RawQuat {
            x: q.x,
            y: q.y,
            z: q.z,
            w: q.w,
        },
    }
}

fn from_raw(pose: RawPose) -> Pose {
    Pose::new(
        Vec3::new(pose.p.x, pose.p.y, pose.p.z),
        Quat::from_xyzw(pose.q.x, pose.q.y, pose.q.z, pose.q.w),
    )
}

fn motion(motion: Motion) -> u32 {
    match motion {
        Motion::Static => 0,
        Motion::Kinematic => 1,
        Motion::Dynamic => 2,
    }
}

fn raw_shape(shape: &Shape) -> RawShape {
    let geometry = &shape.geometry;
    let mut raw = RawShape {
        geometry: CUBOID,
        key: geometry.key(),
        pose: raw_pose(shape.pose),
        half_extents: vec3(Vec3::ZERO),
        radius: 0.0,
        points: std::ptr::null(),
        point_count: 0,
        indices: std::ptr::null(),
        index_count: 0,
        volume: geometry.volume(),
        centroid: vec3(geometry.centroid()),
        density: shape.density,
        friction: shape.friction,
        restitution: shape.restitution,
        rolling_resistance: shape.rolling_resistance,
        category: shape.category,
        mask: shape.mask,
    };
    match geometry.form() {
        Form::Cuboid { half } => raw.half_extents = vec3(*half),
        Form::Sphere { radius } => {
            raw.geometry = SPHERE;
            raw.radius = *radius;
        }
        Form::Hull(hull) => {
            raw.geometry = HULL;
            raw.points = hull.points.as_ptr().cast();
            raw.point_count = hull.points.len() as u32;
        }
        Form::Mesh(mesh) => {
            raw.geometry = MESH;
            raw.points = mesh.vertices.as_ptr().cast();
            raw.point_count = mesh.vertices.len() as u32;
            raw.indices = mesh.indices.as_ptr();
            raw.index_count = mesh.indices.len() as u32;
        }
    }
    raw
}

trait Native {
    const NAME: &'static str;

    fn engine() -> &'static RawEngine;
}

#[derive(Debug)]
pub struct Box3d;

#[derive(Debug)]
pub struct Jolt;

#[derive(Debug)]
pub struct Bullet;

impl Native for Box3d {
    const NAME: &'static str = "box3d";

    fn engine() -> &'static RawEngine {
        box3d_bench_engine()
    }
}

impl Native for Jolt {
    const NAME: &'static str = "jolt";

    fn engine() -> &'static RawEngine {
        jolt_bench_engine()
    }
}

impl Native for Bullet {
    const NAME: &'static str = "bullet";

    fn engine() -> &'static RawEngine {
        bullet_bench_engine()
    }
}

impl Engine for Box3d {
    const NAME: &'static str = <Self as Native>::NAME;

    fn world(settings: &Settings) -> Result<Box<dyn World>> {
        NativeWorld::<Self>::create(settings)
    }
}

impl Engine for Jolt {
    const NAME: &'static str = <Self as Native>::NAME;

    fn world(settings: &Settings) -> Result<Box<dyn World>> {
        NativeWorld::<Self>::create(settings)
    }
}

impl Engine for Bullet {
    const NAME: &'static str = <Self as Native>::NAME;

    fn world(settings: &Settings) -> Result<Box<dyn World>> {
        NativeWorld::<Self>::create(settings)
    }
}

struct NativeWorld<N: Native> {
    world: NonNull<c_void>,
    engine: &'static RawEngine,
    native: PhantomData<N>,
}

impl<N: Native + 'static> NativeWorld<N> {
    fn create(settings: &Settings) -> Result<Box<dyn World>> {
        let engine = N::engine();
        let def = RawWorldDef {
            workers: settings.workers as u32,
            substeps: SUBSTEPS,
            sleeping: settings.sleeping,
            sleep_threshold: settings.sleep_threshold,
            gravity: vec3(GRAVITY),
            max_bodies: settings.capacity as u32,
        };
        let world = unsafe { (engine.create)(&def) };
        let world = NonNull::new(world).ok_or(Self::rejected("world definition"))?;
        Ok(Box::new(Self {
            world,
            engine,
            native: PhantomData,
        }))
    }

    fn rejected(what: &'static str) -> Error {
        Error::Rejected {
            engine: N::NAME,
            what,
        }
    }

    fn as_ptr(&self) -> *const c_void {
        self.world.as_ptr()
    }

    fn at_mut(&mut self) -> *mut c_void {
        self.world.as_ptr()
    }
}

impl<N: Native + 'static> World for NativeWorld<N> {
    fn add_body(&mut self, body: &BodyDef) -> Result<BodyIndex> {
        let shapes: Vec<RawShape> = body.shapes.iter().map(raw_shape).collect();
        let raw = RawBody {
            motion: motion(body.motion),
            pose: raw_pose(body.pose),
            linear_velocity: vec3(body.linear_velocity),
            angular_velocity: vec3(body.angular_velocity),
            sleep: body.sleep,
            shapes: shapes.as_ptr(),
            shape_count: shapes.len() as u32,
        };
        match unsafe { (self.engine.add_body)(self.at_mut(), &raw) } {
            INVALID_BODY => Err(Self::rejected("body")),
            index => Ok(BodyIndex(index)),
        }
    }

    fn add_joint(&mut self, joint: &JointDef) -> Result<()> {
        let (kind, motor) = match joint.kind {
            JointKind::Spherical => (SPHERICAL, None),
            JointKind::Revolute(motor) => (REVOLUTE, motor),
        };
        let raw = RawJoint {
            kind,
            body_a: joint.a.0,
            body_b: joint.b.0,
            frame_a: raw_pose(joint.frame_a),
            frame_b: raw_pose(joint.frame_b),
            motor: motor.is_some(),
            motor_speed: motor.map_or(0.0, |motor| motor.speed),
            max_motor_torque: motor.map_or(0.0, |motor| motor.max_torque),
        };
        match unsafe { (self.engine.add_joint)(self.at_mut(), &raw) } {
            true => Ok(()),
            false => Err(Self::rejected("joint")),
        }
    }

    fn prepare(&mut self) {
        unsafe { (self.engine.prepare)(self.at_mut()) };
    }

    fn move_kinematic(&mut self, body: BodyIndex, target: Pose) {
        unsafe { (self.engine.move_kinematic)(self.at_mut(), body.0, raw_pose(target), TIME_STEP) };
    }

    fn step(&mut self) {
        unsafe { (self.engine.step)(self.at_mut(), TIME_STEP) };
    }

    fn pose(&self, body: BodyIndex) -> Pose {
        from_raw(unsafe { (self.engine.pose)(self.as_ptr(), body.0) })
    }

    fn awake_bodies(&self) -> usize {
        unsafe { (self.engine.awake_bodies)(self.as_ptr()) as usize }
    }
}

impl<N: Native> Drop for NativeWorld<N> {
    fn drop(&mut self) {
        unsafe { (self.engine.destroy)(self.world.as_ptr()) };
    }
}
