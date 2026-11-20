use rikka_physics_math::Transform;

use super::impulses::JointImpulses;
use super::kinds::{JointParams, JointVisitor};
use super::model::JointModel;
use super::spring::Spring;
use crate::graph::Edge;

#[derive(Clone, Copy, Debug)]
pub(crate) struct JointRecord {
    pub params: JointParams,
    pub frame_a: Transform,
    pub frame_b: Transform,
    pub body_a: u32,
    pub body_b: u32,
    pub edges: [Edge; 2],
    pub solver_a: u32,
    pub solver_b: u32,
    pub set: u32,
    pub color: u32,
    pub local: u32,
    pub island: u32,
    pub island_prev: u32,
    pub island_next: u32,
    pub impulses: JointImpulses,
    pub reaction_scale: f32,
    pub softness: Option<Spring>,
    pub collide_connected: bool,
    pub break_force: f32,
    pub break_torque: f32,
    pub force_threshold: f32,
    pub torque_threshold: f32,
    pub over_threshold: bool,
    pub user_data: u64,
    pub revision: u64,
}

impl JointRecord {
    pub(crate) fn reaction(&self) -> (f32, f32) {
        self.params.kind().dispatch(Reaction {
            impulses: &self.impulses,
            inv_h: self.reaction_scale,
        })
    }
}

struct Reaction<'a> {
    impulses: &'a JointImpulses,
    inv_h: f32,
}

impl JointVisitor for Reaction<'_> {
    type Output = (f32, f32);

    fn visit<M: JointModel>(self) -> (f32, f32) {
        M::reaction(self.impulses, self.inv_h)
    }
}
