mod ankle;
mod bend;
mod carry;
mod clear;
mod reach;
mod swing;

use std::sync::Arc;

use animate_body::{Body, HumanPose, Side, Worlds};
use rikka_model_core::glam::Vec3;

use crate::drag::bend::Bend;
use crate::drag::carry::Carry;
use crate::drag::reach::Reach;
use crate::drag::swing::{hinge, pelvis, swing, swivel};
use crate::effector::Pin;
use crate::floor::settle;
use crate::handle::Handle;
use crate::hold::{pinned, standing};
use crate::slack::Slack;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragScope {
    Limb,
    FullBody,
}

pub(crate) struct Start<'a> {
    pub(crate) body: &'a Body,
    pub(crate) pose: &'a HumanPose,
    pub(crate) worlds: &'a Worlds,
    pub(crate) handle: Handle,
    pub(crate) grip: Vec3,
    pub(crate) limits: bool,
}

#[derive(Clone, Debug)]
enum Motion {
    Carry(Carry),
    Pelvis,
    Bend(Box<Bend>),
    Swing,
    Hinge,
    Swivel(Pin),
    Reach(Box<Reach>),
}

fn pinned_hand(pins: &[Pin], side: Side) -> Option<Pin> {
    pins.iter().find(|pin| pin.limb == side.arm()).copied()
}

impl Motion {
    fn new(start: &Start<'_>, pinned: &[Pin], standing: &[Pin]) -> Self {
        match start.handle {
            Handle::Centre => Self::Carry(Carry::new(start, pinned, standing)),
            Handle::LowerBody => Self::Pelvis,
            Handle::UpperBody | Handle::Head => Self::Bend(Box::new(Bend::new(start))),
            Handle::Shoulder(_) | Handle::Toes(_) => Self::Swing,
            Handle::UpperArm(side) => pinned_hand(pinned, side).map_or(Self::Swing, Self::Swivel),
            Handle::Forearm(side) => pinned_hand(pinned, side).map_or(Self::Hinge, Self::Swivel),
            Handle::Hand(side) => Self::Reach(Box::new(Reach::new(start, side.arm(), pinned))),
            Handle::Foot(side) => Self::Reach(Box::new(Reach::new(start, side.leg(), pinned))),
        }
    }

    fn solve(
        &mut self,
        start: &Start<'_>,
        pose: HumanPose,
        travel: Vec3,
        scope: DragScope,
    ) -> HumanPose {
        let target = start.grip + travel;
        match self {
            Self::Carry(carry) => carry.solve(start, pose, target, scope),
            Self::Pelvis => pelvis(start, pose, target),
            Self::Bend(bend) => bend.solve(start, pose, target, scope),
            Self::Swing => swing(start, pose, target),
            Self::Hinge => hinge(start, pose, target),
            Self::Swivel(pin) => swivel(start, pose, target, pin.position),
            Self::Reach(reach) => reach.solve(start, pose, target, scope),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DragSession {
    body: Arc<Body>,
    start: HumanPose,
    start_worlds: Worlds,
    handle: Handle,
    grip: Vec3,
    pinned: Vec<Pin>,
    standing: Vec<Pin>,
    carried: bool,
    slack: Slack,
    limits: bool,
    motion: Motion,
    current: HumanPose,
}

impl DragSession {
    #[must_use]
    pub fn begin(
        body: Arc<Body>,
        pose: HumanPose,
        handle: Handle,
        pins: &[Pin],
        limits: bool,
    ) -> Self {
        let start_worlds = body.fk(&pose);
        let pinned = pinned(handle, pins);
        let standing = standing(&start_worlds, handle, &pinned);
        let grip = handle.point(&body, &start_worlds);
        let start = Start {
            body: &body,
            pose: &pose,
            worlds: &start_worlds,
            handle,
            grip,
            limits,
        };
        let motion = Motion::new(&start, &pinned, &standing);
        Self {
            slack: Slack::new(&body, &pinned, limits),
            body,
            start: pose,
            start_worlds,
            handle,
            grip,
            pinned,
            standing,
            carried: false,
            limits,
            motion,
            current: pose,
        }
    }

    pub fn update(&mut self, travel: Vec3, scope: DragScope) -> HumanPose {
        let carrying = self.handle == Handle::Centre && scope == DragScope::FullBody;
        if self.carried && !carrying {
            let worlds = self.body.fk(&self.current);
            self.standing = standing(&worlds, self.handle, &self.pinned);
            if let Motion::Carry(carry) = &mut self.motion {
                carry.replant(&self.body, &worlds, &self.standing, self.limits);
            }
        }
        self.carried = carrying;
        let start = Start {
            body: &self.body,
            pose: &self.start,
            worlds: &self.start_worlds,
            handle: self.handle,
            grip: self.grip,
            limits: self.limits,
        };
        let solution = self.motion.solve(&start, self.current, travel, scope);
        let slackened = self.slack.keep(&self.body, &self.current, solution);
        let standing: &[Pin] = match carrying {
            true => &[],
            false => &self.standing,
        };
        let pins = self.pinned.iter().chain(standing);
        self.current = settle(&self.body, slackened, pins, &self.start, self.limits);
        self.current
    }

    #[must_use]
    pub fn end(self) -> HumanPose {
        self.current
    }
}
