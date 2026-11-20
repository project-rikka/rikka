use animate_body::{Body, HumanPose, Limb, Worlds};
use rikka_model_core::glam::Quat;

use crate::drag::Start;
use crate::placing::Placing;
use crate::two_bone::Tip;

const GAP: f32 = 0.01;
const KEPT: f32 = 1e-4;
const PACE: f32 = std::f32::consts::PI / 90.0;
const STEPS: usize = 60;
const EDGE_STEPS: usize = 8;

pub(super) fn sunk(body: &Body, worlds: &Worlds, limb: Limb) -> bool {
    body.bulk()
        .holds(worlds, worlds.position(limb.chain()[1]), GAP * body.scale())
}

pub(super) fn clear(start: &Start<'_>, pose: &mut HumanPose, tip: Tip) {
    let body = start.body;
    let worlds = body.fk(pose);
    if !sunk(body, &worlds, tip.limb) {
        return;
    }
    let [upper, _, end] = tip.limb.chain();
    let miss = |worlds: &Worlds| worlds.point(end, tip.grip).distance(tip.goal);
    let allowed = miss(&worlds) + KEPT * body.scale();
    let Some(axis) = (tip.goal - worlds.position(upper)).try_normalize() else {
        return;
    };
    let swung = |angle: f32| {
        let mut placing = Placing::new(body, *pose, start.pose, start.limits);
        let desired = Quat::from_axis_angle(axis, angle) * placing.before().rotation(upper);
        placing.place(upper, desired);
        placing.finish()
    };
    let clear = |pose: &HumanPose| {
        let worlds = body.fk(pose);
        !sunk(body, &worlds, tip.limb) && miss(&worlds) <= allowed
    };
    for step in 1..=STEPS {
        for sign in [1.0, -1.0] {
            let angle = sign * PACE * step as f32;
            if !clear(&swung(angle)) {
                continue;
            }
            let (mut inside, mut outside) = (angle - sign * PACE, angle);
            for _ in 0..EDGE_STEPS {
                let middle = (inside + outside) * 0.5;
                match clear(&swung(middle)) {
                    true => outside = middle,
                    false => inside = middle,
                }
            }
            *pose = swung(outside);
            return;
        }
    }
}
