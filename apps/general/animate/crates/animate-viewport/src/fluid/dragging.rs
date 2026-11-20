use animate_fluid::{Anchor, EmitterSetting};
use animate_stage::{Joint, Lens, Placed};
use rikka_app::prelude::*;
use rikka_model::glam::{Mat4, Vec3};

use crate::easy::Follow;

pub(crate) const REACH: f32 = 12.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SpotDrag {
    setting: EmitterSetting,
    anchor: Anchor,
    frame: Mat4,
    start: Vec3,
    world: Vec3,
    follow: Follow,
    attaching: bool,
}

impl SpotDrag {
    pub(crate) fn new(
        lens: &Lens,
        setting: EmitterSetting,
        anchor: Anchor,
        placed: Placed,
        press: Point,
        modifiers: Modifiers,
    ) -> Option<Self> {
        let screen_point = lens.project(placed.world)?;
        let follow = match modifiers.shift {
            true => Follow::pushed(lens, placed.world, screen_point, press),
            false => Follow::new(lens, placed.world, screen_point, press),
        };
        let mut drag = Self {
            setting,
            anchor,
            frame: placed.frame,
            start: Vec3::ZERO,
            world: placed.world,
            follow,
            attaching: modifiers.control,
        };
        drag.start = drag.offset();
        Some(drag)
    }

    pub(crate) fn setting(&self) -> EmitterSetting {
        self.setting
    }

    pub(crate) fn attaching(&self) -> bool {
        self.attaching
    }

    pub(crate) fn advance(&mut self, lens: &Lens, at: Point) -> Vec3 {
        self.world = self.follow.advance(lens, at);
        self.offset()
    }

    pub(crate) fn offset(&self) -> Vec3 {
        self.anchor
            .offset(self.world, |_, _| Some(self.frame))
            .unwrap_or(self.start)
    }

    pub(crate) fn landing(&self, lens: &Lens, joints: &[Joint], at: Point) -> (Anchor, Vec3) {
        match nearest(lens, joints, at) {
            Some(joint) => (
                Anchor::bone(joint.figure.as_str(), joint.bone.as_str()),
                Vec3::ZERO,
            ),
            None => (Anchor::World, self.world),
        }
    }
}

pub(crate) fn nearest<'a>(lens: &Lens, joints: &'a [Joint], at: Point) -> Option<&'a Joint> {
    joints
        .iter()
        .filter_map(|joint| Some((joint, lens.project(joint.world)?.distance(at))))
        .filter(|(_, away)| *away <= REACH)
        .min_by(|one, two| one.1.total_cmp(&two.1))
        .map(|(joint, _)| joint)
}
