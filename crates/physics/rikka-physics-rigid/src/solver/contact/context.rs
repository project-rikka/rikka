use rikka_physics_collision::{ContactGeometry, ContactHeader, PoolView};
use rikka_physics_core::Shared;

use crate::contact::ContactRecord;
use crate::set::BodyMotion;
use crate::solver::lanes::States;
use crate::solver::softness::Softness;

#[derive(Clone, Copy, Debug)]
pub(crate) struct ContactContext<'a> {
    pub geometry: Shared<'a, ContactGeometry>,
    pub headers: &'a [ContactHeader],
    pub extras: &'a [PoolView],
    pub contacts: &'a [ContactRecord],
    pub motion: Shared<'a, BodyMotion>,
    pub states: States<'a>,
    pub soft: Softness,
    pub static_soft: Softness,
    pub warm_start: f32,
    pub speculative: f32,
    pub split: &'a [f32],
}
