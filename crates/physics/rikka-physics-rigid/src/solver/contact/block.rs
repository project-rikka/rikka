use rikka_physics_math::{Vec3W, Wide};

use crate::solver::lanes::{LaneIndices, LaneKinds, MAX_LANES};
use crate::solver::sym::SymW;
use crate::solver::wide_buffer::Plain;

pub(crate) const NO_CONTACT: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ContactPointW<W: Wide> {
    pub anchor_a: Vec3W<W>,
    pub anchor_b: Vec3W<W>,
    pub base_separation: W,
    pub normal_impulse: W,
    pub total_normal_impulse: W,
    pub normal_mass: W,
    pub lever_arm: W,
    pub relative_velocity: W,
    pub restitution_impulse: W,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ContactBlock<W: Wide> {
    pub points: [ContactPointW<W>; 4],
    pub normal: Vec3W<W>,
    pub tangent1: Vec3W<W>,
    pub tangent2: Vec3W<W>,
    pub center_a: Vec3W<W>,
    pub center_b: Vec3W<W>,
    pub inv_mass_a: W,
    pub inv_mass_b: W,
    pub inv_i_a: SymW<W>,
    pub inv_i_b: SymW<W>,
    pub bias_rate: W,
    pub mass_scale: W,
    pub impulse_scale: W,
    pub friction: W,
    pub restitution: W,
    pub rolling_resistance: W,
    pub tangent_velocity1: W,
    pub tangent_velocity2: W,
    pub tangent_mass_xx: W,
    pub tangent_mass_xy: W,
    pub tangent_mass_yy: W,
    pub twist_mass: W,
    pub rolling_mass: SymW<W>,
    pub friction_impulse1: W,
    pub friction_impulse2: W,
    pub twist_impulse: W,
    pub rolling_impulse: Vec3W<W>,
}

unsafe impl<W: Wide> Plain for ContactBlock<W> {}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct ContactLanes {
    pub index_a: LaneIndices,
    pub index_b: LaneIndices,
    pub contacts: [u32; MAX_LANES],
    pub kinds_a: LaneKinds,
    pub kinds_b: LaneKinds,
    pub point_count: u32,
    pub rolling: bool,
    pub restitution: bool,
}

unsafe impl Plain for ContactLanes {}

impl ContactLanes {
    pub(crate) const EMPTY: Self = Self {
        index_a: [0; MAX_LANES],
        index_b: [0; MAX_LANES],
        contacts: [NO_CONTACT; MAX_LANES],
        kinds_a: LaneKinds::EMPTY,
        kinds_b: LaneKinds::EMPTY,
        point_count: 0,
        rolling: false,
        restitution: false,
    };
}
