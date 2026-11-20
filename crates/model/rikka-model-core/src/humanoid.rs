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

impl HumanBone {
    #[must_use]
    pub fn index(self) -> usize {
        self as usize
    }

    #[must_use]
    pub fn is_required(self) -> bool {
        REQUIRED.contains(&self)
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
