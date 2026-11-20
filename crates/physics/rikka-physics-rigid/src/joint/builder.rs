use rikka_physics_math::Transform;

use super::kinds::JointParams;
use super::spring::Spring;
use crate::BodyId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Anchor {
    World(Transform),
    Local { a: Transform, b: Transform },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct JointBase {
    pub body_a: BodyId,
    pub body_b: BodyId,
    pub anchor: Anchor,
    pub collide_connected: bool,
    pub break_force: f32,
    pub break_torque: f32,
    pub force_threshold: f32,
    pub torque_threshold: f32,
    pub softness: Option<Spring>,
    pub user_data: u64,
}

impl JointBase {
    pub(crate) fn new(body_a: BodyId, body_b: BodyId) -> Self {
        Self {
            body_a,
            body_b,
            anchor: Anchor::Local {
                a: Transform::IDENTITY,
                b: Transform::IDENTITY,
            },
            collide_connected: false,
            break_force: f32::MAX,
            break_torque: f32::MAX,
            force_threshold: f32::MAX,
            torque_threshold: f32::MAX,
            softness: None,
            user_data: 0,
        }
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Joint<T> {
    pub(crate) base: JointBase,
    pub(crate) params: T,
}

impl<T> Joint<T> {
    pub(crate) fn new(body_a: BodyId, body_b: BodyId, params: T) -> Self {
        Self {
            base: JointBase::new(body_a, body_b),
            params,
        }
    }

    pub fn frame(mut self, world: Transform) -> Self {
        self.base.anchor = Anchor::World(world);
        self
    }

    pub fn local_frames(mut self, a: Transform, b: Transform) -> Self {
        self.base.anchor = Anchor::Local { a, b };
        self
    }

    pub fn collide_connected(mut self, collide: bool) -> Self {
        self.base.collide_connected = collide;
        self
    }

    pub fn break_force(mut self, force: f32) -> Self {
        self.base.break_force = force;
        self
    }

    pub fn break_torque(mut self, torque: f32) -> Self {
        self.base.break_torque = torque;
        self
    }

    pub fn force_threshold(mut self, force: f32) -> Self {
        self.base.force_threshold = force;
        self
    }

    pub fn torque_threshold(mut self, torque: f32) -> Self {
        self.base.torque_threshold = torque;
        self
    }

    pub fn softness(mut self, spring: Spring) -> Self {
        self.base.softness = Some(spring);
        self
    }

    pub fn user_data(mut self, user_data: u64) -> Self {
        self.base.user_data = user_data;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointDef {
    pub(crate) base: JointBase,
    pub(crate) params: JointParams,
}
