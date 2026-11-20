pub(crate) const LEAN_ITERATIONS: usize = 16;

pub(crate) const CARRY_ROUNDS: usize = 24;

pub(crate) const STEP: f32 = 1e-3;

pub(crate) const DAMPING: f64 = 0.005;

pub(crate) const PULL: f64 = 1e-5;

pub(crate) const STIFF_LIMB: f64 = 1.0;
pub(crate) const STIFF_SHOULDER: f64 = 3.0;
pub(crate) const STIFF_HEAD: f64 = 4.0;
pub(crate) const STIFF_TORSO: f64 = 6.0;

pub(crate) const WEIGHT_POINT: f32 = 1.0;
pub(crate) const WEIGHT_PIN: f32 = 100.0;

pub(crate) const REACH_SLACK: f32 = 0.999;

pub(crate) const PIN_SLACK: f32 = 1e-4;

pub(crate) const PIN_TURN_SLACK: f32 = 1e-3;
