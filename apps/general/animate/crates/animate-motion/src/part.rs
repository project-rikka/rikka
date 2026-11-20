use std::ops::Range;

use animate_body::{
    EYES, HEAD, HumanBone, HumanPose, LEFT_FINGERS, Limb, RIGHT_FINGERS, TORSO, Worlds, muscles_of,
};
use animate_track::Keyframe;
use rikka_keyframe::Framed;
use rikka_model_core::glam::{Quat, Vec3};

use crate::slopes::Slopes;

const PART_COUNT: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Part {
    Root,
    Torso,
    Head,
    LeftArm,
    RightArm,
    LeftFingers,
    RightFingers,
    LeftLeg,
    RightLeg,
    Gaze,
}

impl Part {
    pub const ALL: [Self; PART_COUNT] = [
        Self::Root,
        Self::Torso,
        Self::Head,
        Self::LeftArm,
        Self::RightArm,
        Self::LeftFingers,
        Self::RightFingers,
        Self::LeftLeg,
        Self::RightLeg,
        Self::Gaze,
    ];

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Torso => "torso",
            Self::Head => "head",
            Self::LeftArm => "leftArm",
            Self::RightArm => "rightArm",
            Self::LeftFingers => "leftFingers",
            Self::RightFingers => "rightFingers",
            Self::LeftLeg => "leftLeg",
            Self::RightLeg => "rightLeg",
            Self::Gaze => "gaze",
        }
    }

    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|part| part.name() == name)
    }

    #[must_use]
    pub fn holding(bone: HumanBone) -> Self {
        let muscles = muscles_of(bone);
        Self::ALL
            .into_iter()
            .find(|part| {
                part.muscles()
                    .is_some_and(|range| range.contains(&muscles.start))
            })
            .unwrap_or(Self::Root)
    }

    #[must_use]
    pub fn limb(self) -> Option<Limb> {
        match self {
            Self::LeftArm => Some(Limb::LeftArm),
            Self::RightArm => Some(Limb::RightArm),
            Self::LeftLeg => Some(Limb::LeftLeg),
            Self::RightLeg => Some(Limb::RightLeg),
            _ => None,
        }
    }

    #[must_use]
    pub fn muscles(self) -> Option<Range<usize>> {
        match self {
            Self::Torso => Some(TORSO),
            Self::Head => Some(HEAD),
            Self::LeftFingers => Some(LEFT_FINGERS),
            Self::RightFingers => Some(RIGHT_FINGERS),
            Self::Gaze => Some(EYES),
            _ => self.limb().map(Limb::muscles),
        }
    }
}

impl From<Limb> for Part {
    fn from(limb: Limb) -> Self {
        match limb {
            Limb::LeftArm => Self::LeftArm,
            Limb::RightArm => Self::RightArm,
            Limb::LeftLeg => Self::LeftLeg,
            Limb::RightLeg => Self::RightLeg,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tag {
    #[default]
    Smooth,
    Ease,
    Hit,
    Linear,
    Step,
    Settle,
}

impl Tag {
    pub const ALL: [Self; 6] = [
        Self::Smooth,
        Self::Ease,
        Self::Hit,
        Self::Linear,
        Self::Step,
        Self::Settle,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Smooth => "smooth",
            Self::Ease => "ease",
            Self::Hit => "hit",
            Self::Linear => "linear",
            Self::Step => "step",
            Self::Settle => "settle",
        }
    }

    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tag| tag.name() == name)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GazeMode {
    #[default]
    Off,
    Point,
    Camera,
}

impl GazeMode {
    pub const ALL: [Self; 3] = [Self::Off, Self::Point, Self::Camera];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Point => "point",
            Self::Camera => "camera",
        }
    }

    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.name() == name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Goal {
    pub position: Vec3,
    pub rotation: Quat,
}

impl Default for Goal {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
        }
    }
}

impl Goal {
    #[must_use]
    pub fn from_worlds(worlds: &Worlds, limb: Limb, scale: f32) -> Self {
        let tip = worlds.joint(limb.chain()[2]);
        Self {
            position: tip.position / scale,
            rotation: tip.rotation,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gaze {
    pub mode: GazeMode,
    pub target: Vec3,
    pub weight: f32,
    pub head_follow: f32,
}

impl Default for Gaze {
    fn default() -> Self {
        Self {
            mode: GazeMode::Off,
            target: Vec3::ZERO,
            weight: 1.0,
            head_follow: 0.5,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PartValues {
    Root {
        position: Vec3,
        rotation: Quat,
    },
    Muscles(Vec<f32>),
    Limb {
        muscles: Vec<f32>,
        pin: bool,
        goal: Goal,
    },
    Gaze {
        eyes: Vec<f32>,
        gaze: Gaze,
    },
}

impl PartValues {
    #[must_use]
    pub fn resting(part: Part, rest: &HumanPose) -> Self {
        crate::PoseSample::resting(rest).values(part)
    }

    #[must_use]
    pub fn fits(&self, part: Part) -> bool {
        let span = part.muscles().map_or(0, |range| range.len());
        match (self, part) {
            (Self::Root { .. }, Part::Root) => true,
            (Self::Gaze { eyes, .. }, Part::Gaze) => eyes.len() == span,
            (
                Self::Muscles(muscles),
                Part::Torso | Part::Head | Part::LeftFingers | Part::RightFingers,
            ) => muscles.len() == span,
            (Self::Limb { muscles, .. }, _) => part.limb().is_some() && muscles.len() == span,
            _ => false,
        }
    }

    pub(crate) fn scalar_count(&self) -> usize {
        match self {
            Self::Root { .. } => 3,
            Self::Muscles(muscles) => muscles.len(),
            Self::Limb { muscles, .. } => muscles.len() + 3,
            Self::Gaze { eyes, .. } => eyes.len() + 5,
        }
    }

    pub(crate) fn scalar(&self, channel: usize) -> f32 {
        let values = match self {
            Self::Root { position, .. } => position.to_array().get(channel).copied(),
            Self::Muscles(muscles) => muscles.get(channel).copied(),
            Self::Limb { muscles, goal, .. } => match channel.checked_sub(muscles.len()) {
                Some(axis) => goal.position.to_array().get(axis).copied(),
                None => muscles.get(channel).copied(),
            },
            Self::Gaze { eyes, gaze } => match channel.checked_sub(eyes.len()) {
                Some(setting) => [
                    gaze.target.x,
                    gaze.target.y,
                    gaze.target.z,
                    gaze.weight,
                    gaze.head_follow,
                ]
                .get(setting)
                .copied(),
                None => eyes.get(channel).copied(),
            },
        };
        values.unwrap_or(0.0)
    }

    pub(crate) fn set_scalar(&mut self, channel: usize, value: f32) {
        let slot = match self {
            Self::Root { position, .. } => position.as_mut().get_mut(channel),
            Self::Muscles(muscles) => muscles.get_mut(channel),
            Self::Limb { muscles, goal, .. } => match channel.checked_sub(muscles.len()) {
                Some(axis) => goal.position.as_mut().get_mut(axis),
                None => muscles.get_mut(channel),
            },
            Self::Gaze { eyes, gaze } => match channel.checked_sub(eyes.len()) {
                Some(setting @ 0..3) => gaze.target.as_mut().get_mut(setting),
                Some(3) => Some(&mut gaze.weight),
                Some(4) => Some(&mut gaze.head_follow),
                Some(_) => None,
                None => eyes.get_mut(channel),
            },
        };
        if let Some(slot) = slot {
            *slot = value;
        }
    }

    pub(crate) fn turn(&self) -> Option<Quat> {
        match self {
            Self::Root { rotation, .. } => Some(*rotation),
            Self::Limb { goal, .. } => Some(goal.rotation),
            _ => None,
        }
    }

    pub(crate) fn set_turn(&mut self, turn: Quat) {
        match self {
            Self::Root { rotation, .. } => *rotation = turn,
            Self::Limb { goal, .. } => goal.rotation = turn,
            _ => {}
        }
    }

    pub(crate) fn pin(&self) -> bool {
        matches!(self, Self::Limb { pin: true, .. })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PartKey {
    pub part: Part,
    pub frame: u32,
    pub tag: Tag,
    pub values: PartValues,
    pub slopes: Option<Slopes>,
}

impl Framed for PartKey {
    type Frame = u32;

    fn frame(&self) -> u32 {
        self.frame
    }
}

impl Keyframe for PartKey {
    type Id = Part;

    fn id(&self) -> &Part {
        &self.part
    }

    fn at_frame(self, frame: u32) -> Self {
        Self { frame, ..self }
    }
}

impl PartKey {
    #[must_use]
    pub fn retagged(self, tag: Tag) -> Self {
        Self {
            tag,
            slopes: None,
            ..self
        }
    }

    #[must_use]
    pub fn fits(&self) -> bool {
        self.values.fits(self.part)
            && self
                .slopes
                .as_ref()
                .is_none_or(|slopes| slopes.fits(&self.values))
    }
}
