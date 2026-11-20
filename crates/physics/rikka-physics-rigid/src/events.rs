use rikka_physics_math::{Transform, Vec3};

use crate::{BodyId, JointId, ShapeId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactBeginEvent {
    pub shape_a: ShapeId,
    pub shape_b: ShapeId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactEndEvent {
    pub shape_a: ShapeId,
    pub shape_b: ShapeId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ContactHitEvent {
    pub shape_a: ShapeId,
    pub shape_b: ShapeId,
    pub point: Vec3,
    pub normal: Vec3,
    pub approach_speed: f32,
    pub material_a: u32,
    pub material_b: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SensorBeginEvent {
    pub sensor: ShapeId,
    pub visitor: ShapeId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SensorEndEvent {
    pub sensor: ShapeId,
    pub visitor: ShapeId,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyMoveEvent {
    pub body: BodyId,
    pub transform: Transform,
    pub user_data: u64,
    pub fell_asleep: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JointEventKind {
    Threshold,
    Broken,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointEvent {
    pub joint: JointId,
    pub kind: JointEventKind,
    pub force: f32,
    pub torque: f32,
    pub user_data: u64,
}

#[derive(Debug, Default)]
pub(crate) struct Events {
    pub contact_begin: Vec<ContactBeginEvent>,
    pub contact_end: Vec<ContactEndEvent>,
    pub contact_hit: Vec<ContactHitEvent>,
    pub body_moves: Vec<BodyMoveEvent>,
    pub joints: Vec<JointEvent>,
    pub sensor_begin: Vec<SensorBeginEvent>,
    pub sensor_end: Vec<SensorEndEvent>,
}

impl Events {
    pub(crate) fn clear(&mut self) {
        self.contact_begin.clear();
        self.contact_end.clear();
        self.contact_hit.clear();
        self.body_moves.clear();
        self.joints.clear();
        self.sensor_begin.clear();
        self.sensor_end.clear();
    }
}
