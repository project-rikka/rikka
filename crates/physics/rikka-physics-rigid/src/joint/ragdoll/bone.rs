const DEGREE: f32 = std::f32::consts::PI / 180.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HumanBone {
    Pelvis,
    Spine,
    Chest,
    Head,
    UpperArmLeft,
    LowerArmLeft,
    UpperArmRight,
    LowerArmRight,
    ThighLeft,
    CalfLeft,
    ThighRight,
    CalfRight,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Socket {
    Ball {
        cone: f32,
        twist: f32,
        friction: f32,
    },
    Hinge {
        limit: f32,
        friction: f32,
    },
}

impl HumanBone {
    pub const ALL: [HumanBone; 12] = [
        HumanBone::Pelvis,
        HumanBone::Spine,
        HumanBone::Chest,
        HumanBone::Head,
        HumanBone::UpperArmLeft,
        HumanBone::LowerArmLeft,
        HumanBone::UpperArmRight,
        HumanBone::LowerArmRight,
        HumanBone::ThighLeft,
        HumanBone::CalfLeft,
        HumanBone::ThighRight,
        HumanBone::CalfRight,
    ];

    pub fn parent(self) -> Option<HumanBone> {
        match self {
            HumanBone::Pelvis => None,
            HumanBone::Spine | HumanBone::ThighLeft | HumanBone::ThighRight => {
                Some(HumanBone::Pelvis)
            }
            HumanBone::Chest => Some(HumanBone::Spine),
            HumanBone::Head | HumanBone::UpperArmLeft | HumanBone::UpperArmRight => {
                Some(HumanBone::Chest)
            }
            HumanBone::LowerArmLeft => Some(HumanBone::UpperArmLeft),
            HumanBone::LowerArmRight => Some(HumanBone::UpperArmRight),
            HumanBone::CalfLeft => Some(HumanBone::ThighLeft),
            HumanBone::CalfRight => Some(HumanBone::ThighRight),
        }
    }

    pub(crate) fn index(self) -> usize {
        self as usize
    }

    pub(crate) fn socket(self) -> Option<Socket> {
        let ball = |cone: f32, twist: f32, friction: f32| Socket::Ball {
            cone: cone * DEGREE,
            twist: twist * DEGREE,
            friction,
        };
        let hinge = |limit: f32, friction: f32| Socket::Hinge {
            limit: limit * DEGREE,
            friction,
        };
        match self {
            HumanBone::Pelvis => None,
            HumanBone::Spine | HumanBone::Chest => Some(ball(35.0, 17.5, 1.0)),
            HumanBone::Head => Some(ball(30.0, 17.5, 1.0)),
            HumanBone::ThighLeft | HumanBone::ThighRight => Some(ball(30.0, 10.0, 1.0)),
            HumanBone::UpperArmLeft | HumanBone::UpperArmRight => Some(ball(45.0, 20.0, 0.8)),
            HumanBone::CalfLeft | HumanBone::CalfRight => Some(hinge(30.0, 1.0)),
            HumanBone::LowerArmLeft | HumanBone::LowerArmRight => Some(hinge(35.0, 0.06)),
        }
    }
}
