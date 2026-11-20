use animate_body::{Body, HumanBone, HumanPose, Worlds};
use rikka_model_core::glam::Vec3;

use crate::drag::{DragScope, Start};
use crate::effector::Pin;
use crate::extent::Extent;
use crate::tuning::CARRY_ROUNDS;

#[derive(Clone, Copy, Debug)]
struct Tether {
    centre: Vec3,
    extent: Extent,
}

fn tethers(body: &Body, worlds: &Worlds, pins: &[Pin], limits: bool) -> Vec<Tether> {
    let hips = worlds.position(HumanBone::Hips);
    pins.iter()
        .map(|pin| Tether {
            centre: pin.position - (worlds.position(pin.limb.chain()[0]) - hips),
            extent: Extent::measure(body, pin.limb, limits),
        })
        .collect()
}

#[derive(Clone, Debug)]
pub(super) struct Carry {
    hips: Vec3,
    pinned: Vec<Tether>,
    standing: Vec<Tether>,
}

impl Carry {
    pub(super) fn new(start: &Start<'_>, pinned: &[Pin], standing: &[Pin]) -> Self {
        let (body, worlds) = (start.body, start.worlds);
        Self {
            hips: worlds.position(HumanBone::Hips),
            pinned: tethers(body, worlds, pinned, start.limits),
            standing: tethers(body, worlds, standing, start.limits),
        }
    }

    pub(super) fn replant(&mut self, body: &Body, worlds: &Worlds, standing: &[Pin], limits: bool) {
        self.standing = tethers(body, worlds, standing, limits);
    }

    fn clamp_to_reach(&self, desired: Vec3, scope: DragScope) -> Vec3 {
        let standing: &[Tether] = match scope {
            DragScope::Limb => &self.standing,
            DragScope::FullBody => &[],
        };
        let mut place = desired;
        for _ in 0..CARRY_ROUNDS {
            let before = place;
            for tether in self.pinned.iter().chain(standing) {
                place = tether.extent.keep(tether.centre, place);
            }
            if place == before {
                break;
            }
        }
        place
    }

    pub(super) fn solve(
        &self,
        start: &Start<'_>,
        mut pose: HumanPose,
        target: Vec3,
        scope: DragScope,
    ) -> HumanPose {
        let place = self.clamp_to_reach(target, scope);
        pose.root_pos = start.pose.root_pos + (place - self.hips) / start.body.scale();
        pose
    }
}
