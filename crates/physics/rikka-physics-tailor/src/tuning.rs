use std::f32::consts::PI;

use serde::{Deserialize, Serialize};

use crate::style::{Curve, Style};

const SWAY_FREEDOM: f32 = 3.0;
const SWAY_REACH: f32 = 1.6;
const SWAY_SPEED: f32 = 1.5;
const MAX_DAMPING: f32 = 0.95;
const MAX_SETTLING: f32 = 0.3;
const MIN_ATTENUATION_SCALE: f32 = 0.5;
const MAX_INERTIA_SCALE: f32 = 2.0;
const EXAGGERATED_SPIN: f32 = 1.0;
const RESPONSE_LIMITS: f32 = 4.0;
const MIN_SMOOTHING: f32 = 0.8;
const WEIGHT_RANGE: f32 = 2.0;
const MAX_BUCKLING: f32 = 0.95;
const STIFF_BUCKLING_SCALE: f32 = 0.4;
const LOOSE_EXTENT_SCALE: f32 = 0.8;
const MIN_EXTENT: f32 = 0.5;
const STIFF_EXTENT: f32 = 0.85;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClothTuning {
    pub sway: f32,
    pub response: f32,
    pub weight: f32,
}

impl ClothTuning {
    pub const NEUTRAL: ClothTuning = ClothTuning {
        sway: 0.5,
        response: 0.5,
        weight: 0.5,
    };

    #[must_use]
    pub fn is_neutral(&self) -> bool {
        self.clamped() == ClothTuning::NEUTRAL
    }

    #[must_use]
    pub fn clamped(&self) -> ClothTuning {
        let slider = |value: f32| {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.5
            }
        };
        ClothTuning {
            sway: slider(self.sway),
            response: slider(self.response),
            weight: slider(self.weight),
        }
    }

    #[must_use]
    pub fn apply(&self, base: &Style) -> Style {
        let tuning = self.clamped();
        if tuning == ClothTuning::NEUTRAL {
            return base.clone();
        }
        let mut style = base.clone();
        sway(&mut style, offset(tuning.sway));
        respond(&mut style, offset(tuning.response));
        weigh(&mut style, offset(tuning.weight));
        style
    }
}

impl Default for ClothTuning {
    fn default() -> ClothTuning {
        ClothTuning::NEUTRAL
    }
}

fn offset(slider: f32) -> f32 {
    slider * 2.0 - 1.0
}

fn sway(style: &mut Style, amount: f32) {
    let freedom = SWAY_FREEDOM.powf(amount);
    let reach = SWAY_REACH.powf(amount);
    style.restore = scaled(&style.restore, 1.0 / freedom, 0.0, 1.0);
    style.pull = scaled(&style.pull, 1.0 / freedom, 0.0, 1.0);
    style.limit_stiffness = (style.limit_stiffness / reach).clamp(0.0, 1.0);
    style.max_distance = scaled(&style.max_distance, reach, 0.0, f32::MAX);
    style.limit = scaled(&style.limit, reach, 0.0, PI);
    style.max_speed *= SWAY_SPEED.powf(amount);
    style.bend = (style.bend / reach).clamp(0.0, 1.0);
    style.compression = (style.compression / freedom).clamp(0.0, 1.0);
    if amount > 0.0 {
        style.buckling = lerp(style.buckling, style.buckling.max(MAX_BUCKLING), amount);
        if style.extent > 0.0 {
            style.extent = lerp(
                style.extent,
                (style.extent * LOOSE_EXTENT_SCALE).max(MIN_EXTENT),
                amount,
            );
        }
        style.damping = scaled(&style.damping, 1.0 / freedom, 0.0, MAX_DAMPING);
        style.attenuation = lerp(
            style.attenuation,
            style.attenuation * MIN_ATTENUATION_SCALE,
            amount,
        );
        style.settling = lerp(style.settling, 0.0, amount);
    } else {
        style.buckling = lerp(
            style.buckling,
            style.buckling * STIFF_BUCKLING_SCALE,
            -amount,
        );
        if style.extent > 0.0 {
            style.extent = lerp(style.extent, style.extent.max(STIFF_EXTENT), -amount);
        }
        style.settling = lerp(style.settling, style.settling.max(MAX_SETTLING), -amount);
    }
}

fn respond(style: &mut Style, amount: f32) {
    let limits = RESPONSE_LIMITS.powf(amount);
    let inertia = &mut style.inertia;
    inertia.carried = 1.0 - scaled_share(1.0 - inertia.carried.clamp(-1.0, 1.0), amount);
    inertia.carried_turn = 1.0 - scaled_share(1.0 - inertia.carried_turn.clamp(-1.0, 1.0), amount);
    inertia.depth_carried = lerp_signed(inertia.depth_carried, amount, 1.0, 0.0);
    inertia.centrifugal = lerp_signed(
        inertia.centrifugal,
        amount,
        0.0,
        (inertia.centrifugal * MAX_INERTIA_SCALE).max(EXAGGERATED_SPIN),
    );
    inertia.speed_limit *= limits;
    inertia.turn_limit *= limits;
    inertia.smoothing = lerp_signed(
        inertia.smoothing,
        amount,
        inertia.smoothing.max(MIN_SMOOTHING),
        0.0,
    );
}

fn scaled_share(share: f32, amount: f32) -> f32 {
    lerp_signed(share, amount, 0.0, MAX_INERTIA_SCALE.max(share))
}

fn weigh(style: &mut Style, amount: f32) {
    let heavier = WEIGHT_RANGE.powf(amount);
    style.gravity *= heavier;
    style.drag = (style.drag / heavier).clamp(0.0, 1.0);
    style.lift = (style.lift / heavier).clamp(0.0, 1.0);
    style.flutter.amplitude /= heavier;
    style.max_speed *= heavier.sqrt();
}

fn lerp_signed(value: f32, amount: f32, low: f32, high: f32) -> f32 {
    if amount < 0.0 {
        lerp(value, low, -amount)
    } else {
        lerp(value, high, amount)
    }
}

fn lerp(value: f32, target: f32, share: f32) -> f32 {
    value * (1.0 - share) + target * share
}

fn scaled(curve: &Curve, scale: f32, low: f32, high: f32) -> Curve {
    Curve::from_points(
        curve
            .points()
            .iter()
            .map(|[depth, value]| [*depth, (value * scale).clamp(low, high)]),
    )
}
