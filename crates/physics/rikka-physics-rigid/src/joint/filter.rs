use rikka_physics_math::Wide;

use super::builder::Joint;
use super::impulses::{IMPULSE_SLOTS, JointImpulses};
use super::kinds::JointParams;
use super::model::{JointFrameW, JointModel, JointPrepareContext, JointSolveContext};
use crate::BodyId;
use crate::solver::lanes::{BodyW, MAX_LANES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilterJoint;

impl FilterJoint {
    pub fn new(body_a: BodyId, body_b: BodyId) -> Joint<FilterJoint> {
        Joint::new(body_a, body_b, FilterJoint)
    }

    pub(crate) fn is_valid(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub(crate) struct FilterModel;

impl JointModel for FilterModel {
    type Params = FilterJoint;
    type Block<W: Wide> = ();

    fn params(all: &JointParams) -> &FilterJoint {
        match all {
            JointParams::Filter(params) => params,
            _ => unreachable!("joint batch mixes kinds"),
        }
    }

    fn layout(_: &FilterJoint) -> u32 {
        0
    }

    #[inline(always)]
    fn prepare<W: Wide>(
        _: &mut (),
        _: &JointFrameW<W>,
        _: &[&FilterJoint; MAX_LANES],
        _: &[&JointImpulses; MAX_LANES],
        _: &JointPrepareContext,
    ) {
    }

    #[inline(always)]
    fn warm_start<W: Wide>(_: &JointFrameW<W>, _: &(), _: &mut BodyW<W>, _: &mut BodyW<W>) {}

    #[inline(always)]
    fn solve<W: Wide>(
        _: &JointFrameW<W>,
        _: &mut (),
        _: &mut BodyW<W>,
        _: &mut BodyW<W>,
        _: &JointSolveContext,
    ) {
    }

    #[inline(always)]
    fn impulses<W: Wide>(_: &()) -> [W; IMPULSE_SLOTS] {
        [W::default(); IMPULSE_SLOTS]
    }

    fn reaction(_: &JointImpulses, _: f32) -> (f32, f32) {
        (0.0, 0.0)
    }
}
