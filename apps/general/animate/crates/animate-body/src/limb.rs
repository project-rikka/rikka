use std::ops::Range;

use rikka_model_core::HumanBone;

use crate::hierarchy::is_right;
use crate::muscle::{LEFT_ARM, LEFT_FINGERS, LEFT_LEG, RIGHT_ARM, RIGHT_FINGERS, RIGHT_LEG};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub const BOTH: [Self; 2] = [Self::Left, Self::Right];

    #[must_use]
    pub fn for_bone(bone: HumanBone) -> Self {
        match is_right(bone) {
            true => Self::Right,
            false => Self::Left,
        }
    }

    #[must_use]
    pub fn other(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
    #[must_use]
    pub fn fingers(self) -> Range<usize> {
        match self {
            Self::Left => LEFT_FINGERS,
            Self::Right => RIGHT_FINGERS,
        }
    }

    #[must_use]
    pub fn shoulder(self) -> HumanBone {
        match self {
            Self::Left => HumanBone::LeftShoulder,
            Self::Right => HumanBone::RightShoulder,
        }
    }

    #[must_use]
    pub fn arm(self) -> Limb {
        match self {
            Self::Left => Limb::LeftArm,
            Self::Right => Limb::RightArm,
        }
    }

    #[must_use]
    pub fn leg(self) -> Limb {
        match self {
            Self::Left => Limb::LeftLeg,
            Self::Right => Limb::RightLeg,
        }
    }

    #[must_use]
    pub fn foot(self) -> HumanBone {
        match self {
            Self::Left => HumanBone::LeftFoot,
            Self::Right => HumanBone::RightFoot,
        }
    }

    #[must_use]
    pub fn toes(self) -> HumanBone {
        match self {
            Self::Left => HumanBone::LeftToes,
            Self::Right => HumanBone::RightToes,
        }
    }

    #[must_use]
    pub fn hand(self) -> HumanBone {
        match self {
            Self::Left => HumanBone::LeftHand,
            Self::Right => HumanBone::RightHand,
        }
    }

    #[must_use]
    pub fn eye(self) -> HumanBone {
        match self {
            Self::Left => HumanBone::LeftEye,
            Self::Right => HumanBone::RightEye,
        }
    }

    #[must_use]
    pub fn finger_bones(self) -> [HumanBone; 15] {
        use HumanBone as B;
        match self {
            Self::Left => [
                B::LeftThumbMetacarpal,
                B::LeftThumbProximal,
                B::LeftThumbDistal,
                B::LeftIndexProximal,
                B::LeftIndexIntermediate,
                B::LeftIndexDistal,
                B::LeftMiddleProximal,
                B::LeftMiddleIntermediate,
                B::LeftMiddleDistal,
                B::LeftRingProximal,
                B::LeftRingIntermediate,
                B::LeftRingDistal,
                B::LeftLittleProximal,
                B::LeftLittleIntermediate,
                B::LeftLittleDistal,
            ],
            Self::Right => [
                B::RightThumbMetacarpal,
                B::RightThumbProximal,
                B::RightThumbDistal,
                B::RightIndexProximal,
                B::RightIndexIntermediate,
                B::RightIndexDistal,
                B::RightMiddleProximal,
                B::RightMiddleIntermediate,
                B::RightMiddleDistal,
                B::RightRingProximal,
                B::RightRingIntermediate,
                B::RightRingDistal,
                B::RightLittleProximal,
                B::RightLittleIntermediate,
                B::RightLittleDistal,
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Limb {
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

impl Limb {
    pub const ALL: [Self; 4] = [Self::LeftArm, Self::RightArm, Self::LeftLeg, Self::RightLeg];

    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub fn side(self) -> Side {
        match self {
            Self::LeftArm | Self::LeftLeg => Side::Left,
            Self::RightArm | Self::RightLeg => Side::Right,
        }
    }

    #[must_use]
    pub fn is_leg(self) -> bool {
        matches!(self, Self::LeftLeg | Self::RightLeg)
    }

    #[must_use]
    pub fn muscles(self) -> Range<usize> {
        match self {
            Self::LeftArm => LEFT_ARM,
            Self::RightArm => RIGHT_ARM,
            Self::LeftLeg => LEFT_LEG,
            Self::RightLeg => RIGHT_LEG,
        }
    }

    #[must_use]
    pub fn chain(self) -> [HumanBone; 3] {
        match self {
            Self::LeftArm => [
                HumanBone::LeftUpperArm,
                HumanBone::LeftLowerArm,
                HumanBone::LeftHand,
            ],
            Self::RightArm => [
                HumanBone::RightUpperArm,
                HumanBone::RightLowerArm,
                HumanBone::RightHand,
            ],
            Self::LeftLeg => [
                HumanBone::LeftUpperLeg,
                HumanBone::LeftLowerLeg,
                HumanBone::LeftFoot,
            ],
            Self::RightLeg => [
                HumanBone::RightUpperLeg,
                HumanBone::RightLowerLeg,
                HumanBone::RightFoot,
            ],
        }
    }
}
