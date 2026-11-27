use rikka_model_core::glam::Vec3;

pub(crate) const ROOT: &str = "Root";
pub(crate) const HIPS: &str = "J_Bip_C_Hips";
pub(crate) const SPINE: &str = "J_Bip_C_Spine";
pub(crate) const CHEST: &str = "J_Bip_C_Chest";
pub(crate) const UPPER_CHEST: &str = "J_Bip_C_UpperChest";
pub(crate) const NECK: &str = "J_Bip_C_Neck";
pub(crate) const HEAD: &str = "J_Bip_C_Head";
#[derive(Clone, Debug)]
pub(crate) struct Joint {
    pub(crate) name: String,
    pub(crate) parent: Option<String>,
    pub(crate) at: Vec3,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Finger {
    pub(crate) name: &'static str,
    pub(crate) joints: [Vec3; 3],
    pub(crate) tip: Vec3,
    pub(crate) radius: f32,
}

struct Placed(&'static str, &'static str, [f32; 3]);

const CENTRE: [Placed; 7] = [
    Placed(ROOT, "", [0.0, 0.0, 0.0]),
    Placed(HIPS, ROOT, [0.0, 0.97, 0.0]),
    Placed(SPINE, HIPS, [0.0, 1.07, 0.0]),
    Placed(CHEST, SPINE, [0.0, 1.2, 0.0]),
    Placed(UPPER_CHEST, CHEST, [0.0, 1.32, 0.0]),
    Placed(NECK, UPPER_CHEST, [0.0, 1.47, -0.01]),
    Placed(HEAD, NECK, [0.0, 1.56, 0.0]),
];

const LEFT: [Placed; 8] = [
    Placed("Shoulder", UPPER_CHEST, [0.03, 1.43, -0.01]),
    Placed("UpperArm", "Shoulder", [0.17, 1.43, -0.02]),
    Placed("LowerArm", "UpperArm", [0.44, 1.43, -0.02]),
    Placed("Hand", "LowerArm", [0.69, 1.43, -0.02]),
    Placed("UpperLeg", HIPS, [0.09, 0.92, 0.0]),
    Placed("LowerLeg", "UpperLeg", [0.09, 0.5, 0.01]),
    Placed("Foot", "LowerLeg", [0.09, 0.08, -0.02]),
    Placed("ToeBase", "Foot", [0.09, 0.025, 0.11]),
];

pub(crate) const TOE_TIP: Vec3 = Vec3::new(0.09, 0.025, 0.18);

const LEFT_FINGERS: [Finger; 5] = [
    Finger {
        name: "Thumb",
        joints: [
            Vec3::new(0.705, 1.42, 0.01),
            Vec3::new(0.73, 1.413, 0.04),
            Vec3::new(0.755, 1.408, 0.058),
        ],
        tip: Vec3::new(0.777, 1.405, 0.07),
        radius: 0.011,
    },
    Finger {
        name: "Index",
        joints: [
            Vec3::new(0.785, 1.43, 0.012),
            Vec3::new(0.83, 1.43, 0.014),
            Vec3::new(0.858, 1.43, 0.015),
        ],
        tip: Vec3::new(0.88, 1.43, 0.015),
        radius: 0.0095,
    },
    Finger {
        name: "Middle",
        joints: [
            Vec3::new(0.79, 1.43, -0.009),
            Vec3::new(0.838, 1.43, -0.009),
            Vec3::new(0.869, 1.43, -0.009),
        ],
        tip: Vec3::new(0.892, 1.43, -0.009),
        radius: 0.0098,
    },
    Finger {
        name: "Ring",
        joints: [
            Vec3::new(0.785, 1.43, -0.03),
            Vec3::new(0.83, 1.43, -0.031),
            Vec3::new(0.859, 1.43, -0.032),
        ],
        tip: Vec3::new(0.88, 1.43, -0.032),
        radius: 0.0092,
    },
    Finger {
        name: "Little",
        joints: [
            Vec3::new(0.775, 1.43, -0.049),
            Vec3::new(0.81, 1.43, -0.052),
            Vec3::new(0.832, 1.43, -0.054),
        ],
        tip: Vec3::new(0.85, 1.43, -0.055),
        radius: 0.0085,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
}

impl Side {
    pub(crate) const BOTH: [Side; 2] = [Side::Left, Side::Right];

    fn letter(self) -> &'static str {
        match self {
            Side::Left => "L",
            Side::Right => "R",
        }
    }

    pub(crate) fn place(self, at: Vec3) -> Vec3 {
        match self {
            Side::Left => at,
            Side::Right => Vec3::new(-at.x, at.y, at.z),
        }
    }

    pub(crate) fn bone(self, role: &str) -> String {
        format!("J_Bip_{}_{role}", self.letter())
    }

    pub(crate) fn finger_bone(self, finger: &str, joint: usize) -> String {
        self.bone(&format!("{finger}{}", joint + 1))
    }

    pub(crate) fn fingers(self) -> [Finger; 5] {
        LEFT_FINGERS.map(|finger| Finger {
            joints: finger.joints.map(|joint| self.place(joint)),
            tip: self.place(finger.tip),
            ..finger
        })
    }
}

fn sided_parent(side: Side, parent: &str) -> String {
    match CENTRE.iter().any(|placed| placed.0 == parent) {
        true => parent.to_owned(),
        false => side.bone(parent),
    }
}

pub(crate) fn joint_at(name: &str) -> Vec3 {
    joints()
        .into_iter()
        .find(|joint| joint.name == name)
        .map_or(Vec3::ZERO, |joint| joint.at)
}

pub(crate) fn joints() -> Vec<Joint> {
    let mut joints: Vec<Joint> = CENTRE
        .iter()
        .map(|Placed(name, parent, at)| Joint {
            name: (*name).to_owned(),
            parent: (!parent.is_empty()).then(|| (*parent).to_owned()),
            at: Vec3::from(*at),
        })
        .collect();
    for side in Side::BOTH {
        joints.extend(LEFT.iter().map(|Placed(role, parent, at)| Joint {
            name: side.bone(role),
            parent: Some(sided_parent(side, parent)),
            at: side.place(Vec3::from(*at)),
        }));
        for finger in side.fingers() {
            for (index, at) in finger.joints.into_iter().enumerate() {
                let parent = match index {
                    0 => side.bone("Hand"),
                    _ => side.finger_bone(finger.name, index - 1),
                };
                joints.push(Joint {
                    name: side.finger_bone(finger.name, index),
                    parent: Some(parent),
                    at,
                });
            }
        }
    }
    joints
}
