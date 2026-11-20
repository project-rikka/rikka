use animate_body::{HumanPose, Limb, Worlds, arc};
use rikka_model_core::glam::{Quat, Vec3};

use crate::drag::Start;
use crate::extent::Extent;
use crate::two_bone::orient;

const LIFTING: f32 = 0.05;
const LIFTED: f32 = 0.4;

#[derive(Clone, Copy, Debug)]
pub(super) struct Ankle {
    limb: Limb,
    rest_shin: Vec3,
    rest_height: f32,
    rise: (f32, f32),
    offset: Quat,
}

fn smoothstep((from, to): (f32, f32), value: f32) -> f32 {
    let along = ((value - from) / (to - from)).clamp(0.0, 1.0);
    along * along * (3.0 - 2.0 * along)
}

fn shin(worlds: &Worlds, limb: Limb) -> Vec3 {
    let [_, lower, foot] = limb.chain();
    worlds.position(foot) - worlds.position(lower)
}

impl Ankle {
    pub(super) fn new(start: &Start<'_>, limb: Limb) -> Self {
        let body = start.body;
        let rest = body.fk(body.rest_pose());
        let foot = limb.chain()[2];
        let leg = Extent::measure(body, limb, start.limits).far;
        let mut ankle = Self {
            limb,
            rest_shin: shin(&rest, limb),
            rest_height: rest.position(foot).y,
            rise: (LIFTING * leg, LIFTED * leg),
            offset: Quat::IDENTITY,
        };
        let worlds = start.worlds;
        let follow = ankle.follow(worlds, worlds.position(foot).y);
        ankle.offset = (follow.inverse() * worlds.rotation(foot)).normalize();
        ankle
    }

    fn follow(&self, worlds: &Worlds, height: f32) -> Quat {
        let lift = smoothstep(self.rise, height - self.rest_height);
        Quat::IDENTITY.slerp(arc(self.rest_shin, shin(worlds, self.limb)), lift)
    }

    pub(super) fn turn(&self, start: &Start<'_>, pose: &mut HumanPose, goal: Vec3) {
        let body = start.body;
        let [_, lower, foot] = self.limb.chain();
        let desired = self.follow(&body.fk(pose), goal.y) * self.offset;
        orient(body, pose, lower, foot, desired.normalize());
        body.clamp_around(pose, start.pose, start.limits);
    }
}
