use rikka_model_core::HumanBone;
use rikka_model_core::glam::Vec3;

use crate::body::Body;
use crate::hand::HandPreset;
use crate::limb::Side;

const PALM: f32 = 0.6;
const THUMB_TIP: usize = 2;
const INDEX_TIP: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pinch {
    pub local: Vec3,
    pub rest: Vec3,
}

impl Body {
    #[must_use]
    pub fn pinch(&self, side: Side) -> Option<Pinch> {
        let hand = side.hand();
        if !self.has(hand) {
            return None;
        }
        let mut pose = *self.rest_pose();
        HandPreset::Pinch.apply(side, &mut pose);
        let worlds = self.fk(&pose);
        let fingers = side.finger_bones();
        let tip = |bone: HumanBone| {
            self.has(bone)
                .then(|| worlds.point(bone, self.reach(bone).unwrap_or(Vec3::ZERO)))
        };
        let rest = match (tip(fingers[THUMB_TIP]), tip(fingers[INDEX_TIP])) {
            (Some(thumb), Some(index)) => (thumb + index) * 0.5,
            (Some(only), None) | (None, Some(only)) => only,
            (None, None) => worlds.point(hand, self.reach(hand).unwrap_or(Vec3::ZERO) * PALM),
        };
        Some(Pinch {
            local: worlds.joint(hand).local(rest),
            rest,
        })
    }
}
