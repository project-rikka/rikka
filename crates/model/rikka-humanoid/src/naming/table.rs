use rikka_model_core::HumanBone;

use super::words::Side;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Limb {
    Shoulder,
    UpperArm,
    LowerArm,
    Hand,
    UpperLeg,
    LowerLeg,
    Foot,
    Toes,
    Eye,
}

pub(crate) const LIMBS: [Limb; 9] = [
    Limb::Shoulder,
    Limb::UpperArm,
    Limb::LowerArm,
    Limb::Hand,
    Limb::UpperLeg,
    Limb::LowerLeg,
    Limb::Foot,
    Limb::Toes,
    Limb::Eye,
];

impl Limb {
    pub(crate) fn keys(self) -> &'static [&'static str] {
        match self {
            Self::Shoulder => &["shoulder", "clavicle", "collar", "肩"],
            Self::UpperArm => &["upperarm", "arm", "uparm", "shldrbend", "腕"],
            Self::LowerArm => &[
                "lowerarm",
                "forearm",
                "forearmbend",
                "lowarm",
                "elbow",
                "ひじ",
                "肘",
            ],
            Self::Hand => &["hand", "wrist", "手首"],
            Self::UpperLeg => &["upperleg", "upleg", "thigh", "thighbend", "hip", "足"],
            Self::LowerLeg => &[
                "lowerleg", "leg", "lowleg", "calf", "shin", "knee", "ひざ", "膝",
            ],
            Self::Foot => &["foot", "ankle", "足首"],
            Self::Toes => &["toes", "toebase", "toe", "ball", "toe0", "足先ex", "つま先"],
            Self::Eye => &["eye", "faceeye", "eyeball", "目"],
        }
    }

    pub(crate) fn is_link(self) -> bool {
        matches!(
            self,
            Self::LowerArm | Self::Hand | Self::LowerLeg | Self::Foot
        )
    }

    pub(crate) fn bone(self, side: Side) -> HumanBone {
        use HumanBone as B;
        match (self, side) {
            (Self::Shoulder, Side::Left) => B::LeftShoulder,
            (Self::Shoulder, Side::Right) => B::RightShoulder,
            (Self::UpperArm, Side::Left) => B::LeftUpperArm,
            (Self::UpperArm, Side::Right) => B::RightUpperArm,
            (Self::LowerArm, Side::Left) => B::LeftLowerArm,
            (Self::LowerArm, Side::Right) => B::RightLowerArm,
            (Self::Hand, Side::Left) => B::LeftHand,
            (Self::Hand, Side::Right) => B::RightHand,
            (Self::UpperLeg, Side::Left) => B::LeftUpperLeg,
            (Self::UpperLeg, Side::Right) => B::RightUpperLeg,
            (Self::LowerLeg, Side::Left) => B::LeftLowerLeg,
            (Self::LowerLeg, Side::Right) => B::RightLowerLeg,
            (Self::Foot, Side::Left) => B::LeftFoot,
            (Self::Foot, Side::Right) => B::RightFoot,
            (Self::Toes, Side::Left) => B::LeftToes,
            (Self::Toes, Side::Right) => B::RightToes,
            (Self::Eye, Side::Left) => B::LeftEye,
            (Self::Eye, Side::Right) => B::RightEye,
        }
    }
}

pub(crate) const HEAD: &[&str] = &["head", "頭"];

pub(crate) const JAW: &[&str] = &["jaw", "jawroot", "顎", "あご"];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Digit {
    Thumb,
    Index,
    Middle,
    Ring,
    Little,
}

pub(crate) const DIGITS: [Digit; 5] = [
    Digit::Thumb,
    Digit::Index,
    Digit::Middle,
    Digit::Ring,
    Digit::Little,
];

pub(crate) const BIPED_FINGER: &str = "finger";

impl Digit {
    pub(crate) fn from_word(word: &str) -> Option<Self> {
        match word {
            "thumb" | "親指" => Some(Self::Thumb),
            "index" | "pointer" | "人指" | "人差指" => Some(Self::Index),
            "middle" | "mid" | "中指" => Some(Self::Middle),
            "ring" | "薬指" => Some(Self::Ring),
            "little" | "pinky" | "pinkie" | "small" | "小指" => Some(Self::Little),
            _ => None,
        }
    }

    pub(crate) fn from_biped(number: u32) -> Option<Self> {
        DIGITS.get(number as usize).copied()
    }

    pub(crate) fn is_thumb(self) -> bool {
        self == Self::Thumb
    }

    pub(crate) fn bones(self, side: Side) -> [HumanBone; 3] {
        use HumanBone as B;
        match (self, side) {
            (Self::Thumb, Side::Left) => [
                B::LeftThumbMetacarpal,
                B::LeftThumbProximal,
                B::LeftThumbDistal,
            ],
            (Self::Index, Side::Left) => [
                B::LeftIndexProximal,
                B::LeftIndexIntermediate,
                B::LeftIndexDistal,
            ],
            (Self::Middle, Side::Left) => [
                B::LeftMiddleProximal,
                B::LeftMiddleIntermediate,
                B::LeftMiddleDistal,
            ],
            (Self::Ring, Side::Left) => [
                B::LeftRingProximal,
                B::LeftRingIntermediate,
                B::LeftRingDistal,
            ],
            (Self::Little, Side::Left) => [
                B::LeftLittleProximal,
                B::LeftLittleIntermediate,
                B::LeftLittleDistal,
            ],
            (Self::Thumb, Side::Right) => [
                B::RightThumbMetacarpal,
                B::RightThumbProximal,
                B::RightThumbDistal,
            ],
            (Self::Index, Side::Right) => [
                B::RightIndexProximal,
                B::RightIndexIntermediate,
                B::RightIndexDistal,
            ],
            (Self::Middle, Side::Right) => [
                B::RightMiddleProximal,
                B::RightMiddleIntermediate,
                B::RightMiddleDistal,
            ],
            (Self::Ring, Side::Right) => [
                B::RightRingProximal,
                B::RightRingIntermediate,
                B::RightRingDistal,
            ],
            (Self::Little, Side::Right) => [
                B::RightLittleProximal,
                B::RightLittleIntermediate,
                B::RightLittleDistal,
            ],
        }
    }
}

pub(crate) fn joint_rank(word: &str) -> Option<u32> {
    match word {
        "metacarpal" => Some(0),
        "proximal" => Some(1),
        "intermediate" => Some(2),
        "distal" => Some(3),
        _ => None,
    }
}

pub(crate) const FINGER_FILLERS: &[&str] = &["hand", "f", "finger"];

pub(crate) const METACARPALS: &[&str] = &["metacarpal", "carpal", "palm"];
