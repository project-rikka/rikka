use crate::node::NodeId;

macro_rules! human_bones {
    ($($variant:ident => $name:literal),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum HumanBone {
            $($variant),*
        }

        impl HumanBone {
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            #[must_use]
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),*
                }
            }

            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }
    };
}

human_bones! {
    Hips => "hips",
    Spine => "spine",
    Chest => "chest",
    UpperChest => "upperChest",
    Neck => "neck",
    Head => "head",
    LeftEye => "leftEye",
    RightEye => "rightEye",
    Jaw => "jaw",
    LeftUpperLeg => "leftUpperLeg",
    LeftLowerLeg => "leftLowerLeg",
    LeftFoot => "leftFoot",
    LeftToes => "leftToes",
    RightUpperLeg => "rightUpperLeg",
    RightLowerLeg => "rightLowerLeg",
    RightFoot => "rightFoot",
    RightToes => "rightToes",
    LeftShoulder => "leftShoulder",
    LeftUpperArm => "leftUpperArm",
    LeftLowerArm => "leftLowerArm",
    LeftHand => "leftHand",
    RightShoulder => "rightShoulder",
    RightUpperArm => "rightUpperArm",
    RightLowerArm => "rightLowerArm",
    RightHand => "rightHand",
    LeftThumbMetacarpal => "leftThumbMetacarpal",
    LeftThumbProximal => "leftThumbProximal",
    LeftThumbDistal => "leftThumbDistal",
    LeftIndexProximal => "leftIndexProximal",
    LeftIndexIntermediate => "leftIndexIntermediate",
    LeftIndexDistal => "leftIndexDistal",
    LeftMiddleProximal => "leftMiddleProximal",
    LeftMiddleIntermediate => "leftMiddleIntermediate",
    LeftMiddleDistal => "leftMiddleDistal",
    LeftRingProximal => "leftRingProximal",
    LeftRingIntermediate => "leftRingIntermediate",
    LeftRingDistal => "leftRingDistal",
    LeftLittleProximal => "leftLittleProximal",
    LeftLittleIntermediate => "leftLittleIntermediate",
    LeftLittleDistal => "leftLittleDistal",
    RightThumbMetacarpal => "rightThumbMetacarpal",
    RightThumbProximal => "rightThumbProximal",
    RightThumbDistal => "rightThumbDistal",
    RightIndexProximal => "rightIndexProximal",
    RightIndexIntermediate => "rightIndexIntermediate",
    RightIndexDistal => "rightIndexDistal",
    RightMiddleProximal => "rightMiddleProximal",
    RightMiddleIntermediate => "rightMiddleIntermediate",
    RightMiddleDistal => "rightMiddleDistal",
    RightRingProximal => "rightRingProximal",
    RightRingIntermediate => "rightRingIntermediate",
    RightRingDistal => "rightRingDistal",
    RightLittleProximal => "rightLittleProximal",
    RightLittleIntermediate => "rightLittleIntermediate",
    RightLittleDistal => "rightLittleDistal",
}

const COUNT: usize = HumanBone::ALL.len();

const REQUIRED: &[HumanBone] = &[
    HumanBone::Hips,
    HumanBone::Spine,
    HumanBone::Head,
    HumanBone::LeftUpperLeg,
    HumanBone::LeftLowerLeg,
    HumanBone::LeftFoot,
    HumanBone::RightUpperLeg,
    HumanBone::RightLowerLeg,
    HumanBone::RightFoot,
    HumanBone::LeftUpperArm,
    HumanBone::LeftLowerArm,
    HumanBone::LeftHand,
    HumanBone::RightUpperArm,
    HumanBone::RightLowerArm,
    HumanBone::RightHand,
];

const LEFT: &str = "left";
const RIGHT: &str = "right";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Side {
    Left,
    Right,
    Center,
}

impl Side {
    #[must_use]
    pub fn opposite(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
            Side::Center => Side::Center,
        }
    }

    #[must_use]
    pub fn sign(self) -> f32 {
        match self {
            Side::Left => 1.0,
            Side::Right => -1.0,
            Side::Center => 0.0,
        }
    }
}

impl HumanBone {
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub fn is_required(self) -> bool {
        REQUIRED.contains(&self)
    }

    #[must_use]
    pub fn side(self) -> Side {
        let name = self.name();
        if name.starts_with(LEFT) {
            Side::Left
        } else if name.starts_with(RIGHT) {
            Side::Right
        } else {
            Side::Center
        }
    }

    #[must_use]
    pub fn mirrored(self) -> HumanBone {
        let (from, to) = match self.side() {
            Side::Left => (LEFT, RIGHT),
            Side::Right => (RIGHT, LEFT),
            Side::Center => return self,
        };
        let rest = &self.name()[from.len()..];
        HumanBone::ALL
            .iter()
            .copied()
            .find(|bone| bone.name().strip_prefix(to) == Some(rest))
            .unwrap_or(self)
    }

    #[must_use]
    pub fn parent(self) -> Option<HumanBone> {
        use HumanBone::*;
        Some(match self {
            Hips => return None,
            Spine | LeftUpperLeg | RightUpperLeg => Hips,
            Chest => Spine,
            UpperChest => Chest,
            Neck | LeftShoulder | RightShoulder => UpperChest,
            Head => Neck,
            LeftEye | RightEye | Jaw => Head,
            LeftLowerLeg => LeftUpperLeg,
            LeftFoot => LeftLowerLeg,
            LeftToes => LeftFoot,
            RightLowerLeg => RightUpperLeg,
            RightFoot => RightLowerLeg,
            RightToes => RightFoot,
            LeftUpperArm => LeftShoulder,
            LeftLowerArm => LeftUpperArm,
            LeftHand => LeftLowerArm,
            RightUpperArm => RightShoulder,
            RightLowerArm => RightUpperArm,
            RightHand => RightLowerArm,
            LeftThumbMetacarpal | LeftIndexProximal | LeftMiddleProximal | LeftRingProximal
            | LeftLittleProximal => LeftHand,
            LeftThumbProximal => LeftThumbMetacarpal,
            LeftThumbDistal => LeftThumbProximal,
            LeftIndexIntermediate => LeftIndexProximal,
            LeftIndexDistal => LeftIndexIntermediate,
            LeftMiddleIntermediate => LeftMiddleProximal,
            LeftMiddleDistal => LeftMiddleIntermediate,
            LeftRingIntermediate => LeftRingProximal,
            LeftRingDistal => LeftRingIntermediate,
            LeftLittleIntermediate => LeftLittleProximal,
            LeftLittleDistal => LeftLittleIntermediate,
            RightThumbMetacarpal | RightIndexProximal | RightMiddleProximal | RightRingProximal
            | RightLittleProximal => RightHand,
            RightThumbProximal => RightThumbMetacarpal,
            RightThumbDistal => RightThumbProximal,
            RightIndexIntermediate => RightIndexProximal,
            RightIndexDistal => RightIndexIntermediate,
            RightMiddleIntermediate => RightMiddleProximal,
            RightMiddleDistal => RightMiddleIntermediate,
            RightRingIntermediate => RightRingProximal,
            RightRingDistal => RightRingIntermediate,
            RightLittleIntermediate => RightLittleProximal,
            RightLittleDistal => RightLittleIntermediate,
        })
    }
}

impl std::fmt::Display for HumanBone {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Humanoid {
    bones: [Option<NodeId>; COUNT],
}

impl Default for Humanoid {
    fn default() -> Self {
        Self {
            bones: [None; COUNT],
        }
    }
}

impl Humanoid {
    #[must_use]
    pub fn bone(&self, bone: HumanBone) -> Option<NodeId> {
        self.bones[bone.index()]
    }

    pub fn set_bone(&mut self, bone: HumanBone, node: Option<NodeId>) {
        self.bones[bone.index()] = node;
    }

    pub fn bones(&self) -> impl Iterator<Item = (HumanBone, NodeId)> + '_ {
        HumanBone::ALL
            .iter()
            .filter_map(|bone| Some((*bone, self.bones[bone.index()]?)))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bones.iter().flatten().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn missing(&self) -> impl Iterator<Item = HumanBone> + '_ {
        REQUIRED
            .iter()
            .copied()
            .filter(|bone| self.bones[bone.index()].is_none())
    }
}
