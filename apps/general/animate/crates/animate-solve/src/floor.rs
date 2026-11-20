use animate_body::{Body, HumanPose, Side, Worlds};
use rikka_model_core::glam::Vec3;

use crate::effector::Pin;
use crate::hold::keep;
use crate::two_bone::reach;

const SLACK: f32 = 1e-5;

pub fn ground(body: &Body, pose: &mut HumanPose) {
    let worlds = body.fk(pose);
    let mut depths = heights(body, &worlds);
    let mut reached = false;
    for (side, depth) in Side::BOTH.into_iter().zip(depths) {
        if depth >= 0.0 {
            continue;
        }
        let ankle = worlds.joint(side.foot());
        reach(
            body,
            pose,
            side.leg(),
            ankle.position + Vec3::Y * -depth,
            Some(ankle.rotation),
            1.0,
        );
        reached = true;
    }
    if reached {
        depths = heights(body, &body.fk(pose));
    }
    let depth = depths.into_iter().fold(f32::MAX, f32::min);
    if depth < -SLACK {
        pose.root_pos.y -= depth / body.scale();
    }
}

fn heights(body: &Body, worlds: &Worlds) -> [f32; 2] {
    Side::BOTH.map(|side| body.sole_height(worlds, side))
}

pub(crate) fn settle<'a>(
    body: &Body,
    mut pose: HumanPose,
    pins: impl IntoIterator<Item = &'a Pin>,
    around: &HumanPose,
    limits: bool,
) -> HumanPose {
    ground(body, &mut pose);
    for pin in pins {
        keep(body, &mut pose, pin, around, limits);
    }
    body.clamp_around(&mut pose, around, limits);
    pose
}
