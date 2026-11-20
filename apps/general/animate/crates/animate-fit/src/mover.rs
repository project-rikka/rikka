use animate_body::{Body, Dof, HumanPose, Limb, Side, Worlds, muscle_of, muscles};
use animate_solve::reach;
use rikka_model_core::glam::{Quat, Vec3};

use crate::physique::Physique;
use crate::region::Region;
use crate::scene::Scene;

pub(crate) const MOST: usize = 4;
pub(crate) const STILL: f32 = 1e-3;

const ARM_TURNS: [Dof; MOST] = [Dof::Bend, Dof::Side, Dof::Twist, Dof::Bend];
const ARM_COSTS: [f32; MOST] = [1.0, 1.0, 2.0, 1.5];
const LEG_COSTS: [f32; MOST] = [1.0, 1.0, 0.0, 0.0];
const FARTHEST_TURN: f32 = 0.6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mover {
    Arm(Side),
    Legs,
}

#[derive(Clone, Copy, Debug)]
struct Swivel {
    limb: Limb,
    axis: Vec3,
    upper: Quat,
    parent: Quat,
    end: Vec3,
    turn: Quat,
}

#[derive(Clone, Debug)]
pub(crate) struct Prepared {
    mover: Mover,
    base: HumanPose,
    worlds: Worlds,
    swivels: Vec<Swivel>,
}

impl Mover {
    pub(crate) const ALL: [Self; 3] = [Self::Legs, Self::Arm(Side::Left), Self::Arm(Side::Right)];

    pub(crate) fn index(self) -> usize {
        match self {
            Self::Legs => 0,
            Self::Arm(side) => 1 + side as usize,
        }
    }

    pub(crate) fn dims(self) -> usize {
        match self {
            Self::Arm(_) => 4,
            Self::Legs => 2,
        }
    }

    pub(crate) fn moves(self, region: Region) -> bool {
        match self {
            Self::Arm(side) => region.is_arm() && region.side() == Some(side),
            Self::Legs => region.is_leg(),
        }
    }

    pub(crate) fn costs(self) -> [f32; MOST] {
        match self {
            Self::Arm(_) => ARM_COSTS,
            Self::Legs => LEG_COSTS,
        }
    }

    pub(crate) fn prepare(self, body: &Body, base: &HumanPose) -> Prepared {
        let worlds = body.fk(base);
        let limbs: &[Limb] = match self {
            Self::Arm(_) => &[],
            Self::Legs => &[Limb::LeftLeg, Limb::RightLeg],
        };
        let swivels = limbs
            .iter()
            .map(|limb| {
                let [upper, _, end] = limb.chain();
                let tip = worlds.joint(end);
                Swivel {
                    limb: *limb,
                    axis: (tip.position - worlds.position(upper)).normalize_or(Vec3::NEG_Y),
                    upper: worlds.rotation(upper),
                    parent: body
                        .parent(upper)
                        .map_or(Quat::IDENTITY, |parent| worlds.rotation(parent)),
                    end: tip.position,
                    turn: tip.rotation,
                }
            })
            .collect();
        Prepared {
            mover: self,
            base: *base,
            worlds,
            swivels,
        }
    }

    pub(crate) fn apply(self, body: &Body, pose: &mut HumanPose, x: &[f32; MOST]) {
        if x.iter().all(|value| *value == 0.0) {
            return;
        }
        match self {
            Self::Arm(side) => turn_arm(side, pose, x),
            Self::Legs => self.prepare(body, pose).turn_legs(body, pose, x),
        }
    }
}

impl Prepared {
    pub(crate) fn scene<'a>(
        &self,
        physique: &'a Physique,
        body: &Body,
        x: &[f32; MOST],
    ) -> Scene<'a> {
        if x.iter().all(|value| *value == 0.0) {
            return Scene::new(physique, self.worlds.clone());
        }
        let mut pose = self.base;
        match self.mover {
            Mover::Arm(side) => turn_arm(side, &mut pose, x),
            Mover::Legs => self.turn_legs(body, &mut pose, x),
        }
        Scene::new(physique, body.fk(&pose))
    }

    pub(crate) fn wrist(&self, scene: &Scene<'_>) -> Vec3 {
        match self.mover {
            Mover::Arm(side) => {
                let hand = side.arm().chain()[2];
                scene.position(hand) - self.worlds.position(hand)
            }
            Mover::Legs => Vec3::ZERO,
        }
    }

    fn turn_legs(&self, body: &Body, pose: &mut HumanPose, x: &[f32; MOST]) {
        for (swivel, angle) in self.swivels.iter().zip(x) {
            if *angle == 0.0 {
                continue;
            }
            let [upper, ..] = swivel.limb.chain();
            let rotation = Quat::from_axis_angle(swivel.axis, *angle) * swivel.upper;
            body.set_local_rotation(
                upper,
                (swivel.parent.inverse() * rotation).normalize(),
                pose,
            );
            reach(body, pose, swivel.limb, swivel.end, Some(swivel.turn), 1.0);
        }
    }
}

fn turn_arm(side: Side, pose: &mut HumanPose, x: &[f32; MOST]) {
    let [upper, lower, _] = side.arm().chain();
    let bones = [upper, upper, upper, lower];
    for ((bone, dof), angle) in bones.into_iter().zip(ARM_TURNS).zip(x) {
        let Some(index) = muscle_of(bone, dof) else {
            continue;
        };
        let muscle = &muscles()[index];
        let value = pose.muscles[index];
        let (low, high) = muscle.bounds_around(false, value);
        pose.muscles[index] = muscle.value(muscle.angle(value) + angle).clamp(low, high);
    }
}

pub(crate) fn limited(x: [f32; MOST]) -> [f32; MOST] {
    x.map(|value| value.clamp(-FARTHEST_TURN, FARTHEST_TURN))
}
