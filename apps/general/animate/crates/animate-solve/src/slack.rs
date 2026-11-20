use animate_body::{Body, HumanBone, HumanPose, Worlds};
use rikka_model_core::glam::Vec3;

use crate::effector::Pin;
use crate::extent::Extent;

const STEPS: usize = 10;
const ROOM: f32 = 1e-4;

#[derive(Clone, Debug)]
pub(crate) struct Slack {
    reaches: Vec<(HumanBone, Vec3, Extent)>,
    room: f32,
}

fn blend(from: &HumanPose, to: &HumanPose, share: f32) -> HumanPose {
    let mut blended = *from;
    for (value, (one, other)) in blended
        .muscles
        .iter_mut()
        .zip(from.muscles.iter().zip(to.muscles))
    {
        *value = one + (other - one) * share;
    }
    blended.root_pos = from.root_pos.lerp(to.root_pos, share);
    blended.root_rot = from.root_rot.slerp(to.root_rot, share).normalize();
    blended
}

impl Slack {
    pub(crate) fn new(body: &Body, pins: &[Pin], limits: bool) -> Self {
        Self {
            reaches: pins
                .iter()
                .map(|pin| {
                    let extent = Extent::measure(body, pin.limb, limits);
                    (pin.limb.chain()[0], pin.position, extent)
                })
                .collect(),
            room: ROOM * body.scale(),
        }
    }

    fn excess(&self, worlds: &Worlds) -> f32 {
        self.reaches
            .iter()
            .map(|(root, pin, extent)| {
                let apart = worlds.position(*root).distance(*pin);
                (apart - extent.far).max(extent.near - apart).max(0.0)
            })
            .fold(0.0, f32::max)
    }

    pub(crate) fn keep(&self, body: &Body, from: &HumanPose, to: HumanPose) -> HumanPose {
        if self.reaches.is_empty() {
            return to;
        }
        let allowed = self.excess(&body.fk(from)) + self.room;
        let fits = |pose: &HumanPose| self.excess(&body.fk(pose)) <= allowed;
        if fits(&to) {
            return to;
        }
        let (mut inside, mut outside) = (0.0, 1.0);
        for _ in 0..STEPS {
            let middle = (inside + outside) * 0.5;
            match fits(&blend(from, &to, middle)) {
                true => inside = middle,
                false => outside = middle,
            }
        }
        blend(from, &to, inside)
    }
}
