use rikka_physics_math::{QuatW, Vec3W, Wide};

use super::impulses::{IMPULSE_SLOTS, JointImpulses};
use super::kinds::JointParams;
use crate::solver::lanes::{BodyW, MAX_LANES};
use crate::solver::sym::SymW;
use crate::solver::wide_buffer::Plain;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct JointFrameW<W: Wide> {
    pub anchor_a: Vec3W<W>,
    pub anchor_b: Vec3W<W>,
    pub rotation_a: QuatW<W>,
    pub rotation_b: QuatW<W>,
    pub delta_center: Vec3W<W>,
    pub inv_mass_a: W,
    pub inv_mass_b: W,
    pub inv_i_a: SymW<W>,
    pub inv_i_b: SymW<W>,
    pub bias_rate: W,
    pub mass_scale: W,
    pub impulse_scale: W,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct JointPrepareContext {
    pub h: f32,
    pub inv_h: f32,
    pub max_spring_hertz: f32,
    pub warm_start: f32,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct JointSolveContext {
    pub inv_h: f32,
    pub use_bias: bool,
}

pub(crate) trait JointModel: 'static {
    type Params: Copy + std::fmt::Debug + Send + Sync;
    type Block<W: Wide>: Copy + std::fmt::Debug;

    fn params(all: &JointParams) -> &Self::Params;

    fn layout(params: &Self::Params) -> u32;

    fn prepare<W: Wide>(
        block: &mut Self::Block<W>,
        frame: &JointFrameW<W>,
        params: &[&Self::Params; MAX_LANES],
        impulses: &[&JointImpulses; MAX_LANES],
        ctx: &JointPrepareContext,
    );

    fn warm_start<W: Wide>(
        frame: &JointFrameW<W>,
        block: &Self::Block<W>,
        a: &mut BodyW<W>,
        b: &mut BodyW<W>,
    );

    fn solve<W: Wide>(
        frame: &JointFrameW<W>,
        block: &mut Self::Block<W>,
        a: &mut BodyW<W>,
        b: &mut BodyW<W>,
        ctx: &JointSolveContext,
    );

    fn impulses<W: Wide>(block: &Self::Block<W>) -> [W; IMPULSE_SLOTS];

    fn reaction(impulses: &JointImpulses, inv_h: f32) -> (f32, f32);
}

#[repr(C)]
pub(crate) struct JointBlock<W: Wide, M: JointModel> {
    pub frame: JointFrameW<W>,
    pub model: M::Block<W>,
}

impl<W: Wide, M: JointModel> Clone for JointBlock<W, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<W: Wide, M: JointModel> Copy for JointBlock<W, M> {}

impl<W: Wide, M: JointModel> std::fmt::Debug for JointBlock<W, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JointBlock")
            .field("frame", &self.frame)
            .field("model", &self.model)
            .finish()
    }
}

unsafe impl<W: Wide, M: JointModel> Plain for JointBlock<W, M> {}
