use animate_body::{HumanPose, Limb};
use rikka_model_core::glam::Vec3;

use crate::dls::Solver;
use crate::drag::ankle::Ankle;
use crate::drag::clear::{clear, sunk};
use crate::drag::{DragScope, Start};
use crate::effector::{Goal, Pin};
use crate::extent::Extent;
use crate::scope::{Region, bending};
use crate::tuning::{LEAN_ITERATIONS, WEIGHT_POINT};
use crate::two_bone::{Tip, follow};

#[derive(Clone, Debug)]
enum Kind {
    Arm {
        lean: Solver,
        goals: Vec<Goal>,
        far: f32,
        clear: bool,
    },
    Leg(Ankle),
}

#[derive(Clone, Debug)]
pub(super) struct Reach {
    limb: Limb,
    grip: Vec3,
    kind: Kind,
}

impl Reach {
    pub(super) fn new(start: &Start<'_>, limb: Limb, pins: &[Pin]) -> Self {
        let body = start.body;
        let [root, _, tip] = limb.chain();
        let grip = start.worlds.joint(tip).local(start.grip);
        let kind = match limb.is_leg() {
            true => Kind::Leg(Ankle::new(start, limb)),
            false => {
                let far = Extent::measure(body, limb, start.limits).far + grip.length();
                let reaching = Goal::Within {
                    bone: root,
                    target: Vec3::ZERO,
                    radius: far,
                    weight: WEIGHT_POINT,
                };
                let arm_pins = pins
                    .iter()
                    .filter(|pin| !pin.limb.is_leg())
                    .map(|pin| pin.clamped(body, start.limits));
                Kind::Arm {
                    lean: Solver::new(bending(
                        body,
                        &[Region::Torso, Region::Shoulder(limb.side())],
                    )),
                    goals: std::iter::once(reaching).chain(arm_pins).collect(),
                    far,
                    clear: !sunk(body, start.worlds, limb),
                }
            }
        };
        Self { limb, grip, kind }
    }

    pub(super) fn solve(
        &mut self,
        start: &Start<'_>,
        mut pose: HumanPose,
        target: Vec3,
        scope: DragScope,
    ) -> HumanPose {
        let body = start.body;
        let tip = Tip {
            limb: self.limb,
            grip: self.grip,
            goal: target,
            turn: None,
        };
        match &mut self.kind {
            Kind::Arm {
                lean,
                goals,
                far,
                clear: keeps_clear,
            } => {
                let root = start.worlds.position(self.limb.chain()[0]);
                let leaned = match scope == DragScope::FullBody && root.distance(tip.goal) > *far {
                    true => {
                        goals[0].retarget(tip.goal);
                        lean.solve(
                            body,
                            *start.pose,
                            start.pose,
                            goals,
                            start.limits,
                            LEAN_ITERATIONS,
                        )
                    }
                    false => *start.pose,
                };
                lean.carry(&leaned, &mut pose);
                follow(body, &mut pose, tip, start.pose, start.limits);
                if *keeps_clear {
                    clear(start, &mut pose, tip);
                }
            }
            Kind::Leg(ankle) => {
                follow(body, &mut pose, tip, start.pose, start.limits);
                ankle.turn(start, &mut pose, tip.goal);
            }
        }
        pose
    }
}
