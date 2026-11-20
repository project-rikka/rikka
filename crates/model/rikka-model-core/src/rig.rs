use std::any::Any;

use glam::Mat4;

use crate::model::Model;
use crate::node::NodeId;
use crate::tints::Tints;
use crate::transform::Transform;

#[derive(Debug)]
pub struct PoseState<'a> {
    pub order: &'a [NodeId],
    pub locals: &'a mut [Transform],
    pub world: &'a mut [Mat4],
    pub weights: &'a mut [Vec<f32>],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    BeforePhysics,
    AfterPhysics,
}

pub trait Rig: Any + Send + Sync + std::fmt::Debug {
    fn evaluate(&mut self, model: &Model, transform: Mat4, state: PoseState<'_>, phase: Phase);

    fn duplicate(&self) -> Box<dyn Rig>;

    fn tints(&self) -> Option<&Tints> {
        None
    }
}
