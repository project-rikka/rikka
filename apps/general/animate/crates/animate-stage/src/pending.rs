use animate_body::{HumanPose, MorphWeights};
use animate_cast::{Bone, FigureId, MmdAxis};
use animate_motion::{PoseSample, Take};
use rikka_model::glam::{Quat, Vec3};
use rustc_hash::FxHashMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Local {
    pub translation: Vec3,
    pub rotation: Quat,
}

impl Default for Local {
    fn default() -> Self {
        Self {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

impl Local {
    pub fn playback(take: &Take, bone: &str, frame: u32) -> Self {
        take.bones.value_at(bone, frame as f32).map_or_else(
            Self::default,
            |(translation, rotation)| Self {
                translation,
                rotation,
            },
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Posture {
    pub pose_sample: PoseSample,
    pub shown: HumanPose,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PendingPose {
    Bones(FxHashMap<usize, Local>),
    Human(Box<Posture>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub figure: FigureId,
    pub frame: u32,
    pub pose: Option<PendingPose>,
    pub morphs: MorphWeights,
}

impl Pending {
    pub fn new(figure: FigureId, frame: u32) -> Self {
        Self {
            figure,
            frame,
            pose: None,
            morphs: MorphWeights::default(),
        }
    }

    pub fn bones(&self) -> Option<&FxHashMap<usize, Local>> {
        match &self.pose {
            Some(PendingPose::Bones(bones)) => Some(bones),
            _ => None,
        }
    }

    pub fn posture(&self) -> Option<&Posture> {
        match &self.pose {
            Some(PendingPose::Human(posture)) => Some(posture),
            _ => None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.pose.is_none() && self.morphs.is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Space {
    #[default]
    Global,
    Local,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Nudge {
    Turn(MmdAxis, f32),
    Slide(MmdAxis, f32),
}

pub fn nudged(local: Local, nudge: Nudge, space: Space, parent: Quat, bone: &Bone) -> Local {
    match nudge {
        Nudge::Turn(axis, radians) => {
            let turn = axis.turn(radians);
            let rotation = match (space, bone.fixed_axis) {
                (Space::Global, Some(fixed)) => {
                    let along = (parent * fixed).dot(turn_axis(turn)).signum();
                    Quat::from_axis_angle(fixed.normalize_or(Vec3::X), radians * along)
                        * local.rotation
                }
                (Space::Local, Some(fixed)) => {
                    let along = fixed.dot(turn_axis(turn)).signum();
                    local.rotation
                        * Quat::from_axis_angle(fixed.normalize_or(Vec3::X), radians * along)
                }
                (Space::Global, None) => {
                    let frame = parent * bone.rest_rotation;
                    frame.inverse() * turn * frame * local.rotation
                }
                (Space::Local, None) => local.rotation * turn,
            };
            Local {
                rotation: rotation.normalize(),
                ..local
            }
        }
        Nudge::Slide(axis, units) => {
            let slide = axis.slide(units);
            let along = match space {
                Space::Global => parent.inverse() * slide,
                Space::Local => bone.rest_rotation * local.rotation * slide,
            };
            Local {
                translation: local.translation + along,
                ..local
            }
        }
    }
}

fn turn_axis(turn: Quat) -> Vec3 {
    let (axis, _) = turn.to_axis_angle();
    axis
}
