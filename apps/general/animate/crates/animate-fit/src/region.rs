use animate_body::{HumanBone, Side};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Region {
    Torso,
    Head,
    UpperArm(Side),
    LowerArm(Side),
    Hand(Side),
    UpperLeg(Side),
    LowerLeg(Side),
    Foot(Side),
}

impl Region {
    pub(crate) fn for_bone(bone: HumanBone) -> Self {
        use HumanBone as B;
        let side = Side::for_bone(bone);
        match bone {
            B::Hips | B::Spine | B::Chest | B::UpperChest | B::LeftShoulder | B::RightShoulder => {
                Self::Torso
            }
            B::Neck | B::Head | B::Jaw | B::LeftEye | B::RightEye => Self::Head,
            B::LeftUpperArm | B::RightUpperArm => Self::UpperArm(side),
            B::LeftLowerArm | B::RightLowerArm => Self::LowerArm(side),
            B::LeftUpperLeg | B::RightUpperLeg => Self::UpperLeg(side),
            B::LeftLowerLeg | B::RightLowerLeg => Self::LowerLeg(side),
            B::LeftFoot | B::RightFoot | B::LeftToes | B::RightToes => Self::Foot(side),
            _ => Self::Hand(side),
        }
    }

    fn index(self) -> usize {
        let sided = |side: Side, base: usize| {
            base + match side {
                Side::Left => 0,
                Side::Right => 1,
            }
        };
        match self {
            Self::Torso => 0,
            Self::Head => 1,
            Self::UpperArm(side) => sided(side, 2),
            Self::LowerArm(side) => sided(side, 4),
            Self::Hand(side) => sided(side, 6),
            Self::UpperLeg(side) => sided(side, 8),
            Self::LowerLeg(side) => sided(side, 10),
            Self::Foot(side) => sided(side, 12),
        }
    }

    pub(crate) fn bit(self) -> u16 {
        1 << self.index()
    }

    pub(crate) fn side(self) -> Option<Side> {
        match self {
            Self::Torso | Self::Head => None,
            Self::UpperArm(side)
            | Self::LowerArm(side)
            | Self::Hand(side)
            | Self::UpperLeg(side)
            | Self::LowerLeg(side)
            | Self::Foot(side) => Some(side),
        }
    }

    pub(crate) fn is_arm(self) -> bool {
        matches!(self, Self::UpperArm(_) | Self::LowerArm(_) | Self::Hand(_))
    }

    pub(crate) fn is_leg(self) -> bool {
        matches!(self, Self::UpperLeg(_) | Self::LowerLeg(_) | Self::Foot(_))
    }

    pub(crate) fn avoids(self) -> u16 {
        let legs = |side: Side| Self::UpperLeg(side).bit() | Self::LowerLeg(side).bit();
        let both_legs = legs(Side::Left) | legs(Side::Right);
        match self {
            Self::UpperArm(_) => Self::Torso.bit(),
            Self::LowerArm(_) | Self::Hand(_) => Self::Torso.bit() | Self::Head.bit() | both_legs,
            Self::LowerLeg(side) => legs(side.other()),
            Self::Torso | Self::Head | Self::UpperLeg(_) | Self::Foot(_) => 0,
        }
    }
}
