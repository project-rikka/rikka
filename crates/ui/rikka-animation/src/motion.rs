use crate::duration::DurationToken;
use crate::easing::Easing;
use crate::scheme::{MotionScheme, Property, Speed};
use crate::spring::Spring;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Motion {
    #[default]
    Emphasized,
    Standard,
    Custom {
        easing: Easing,
        duration: DurationToken,
    },
    CustomSpring(Spring),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Curve {
    Spring(Spring),
    Tween {
        easing: Easing,
        duration: DurationToken,
    },
}

impl Curve {
    pub const fn duration(self) -> Option<DurationToken> {
        match self {
            Self::Tween { duration, .. } => Some(duration),
            Self::Spring(_) => None,
        }
    }

    #[must_use]
    pub fn launch_speed(self, distance: f32) -> f32 {
        match self {
            Self::Spring(spring) => spring.launch_speed(distance),
            Self::Tween { .. } => 0.0,
        }
    }
}

impl Motion {
    pub const fn curve(self, property: Property) -> Curve {
        self.curve_at(property, Speed::Default)
    }

    pub const fn curve_at(self, property: Property, speed: Speed) -> Curve {
        match self {
            Self::Emphasized => Curve::Spring(MotionScheme::Expressive.spring(property, speed)),
            Self::Standard => Curve::Spring(MotionScheme::Standard.spring(property, speed)),
            Self::Custom { easing, duration } => Curve::Tween { easing, duration },
            Self::CustomSpring(spring) => Curve::Spring(spring),
        }
    }

    pub const fn enter(duration: DurationToken) -> Self {
        Self::Custom {
            easing: Easing::EmphasizedDecel,
            duration,
        }
    }

    pub const fn exit(duration: DurationToken) -> Self {
        Self::Custom {
            easing: Easing::EmphasizedAccel,
            duration,
        }
    }
}

impl From<Spring> for Motion {
    fn from(spring: Spring) -> Self {
        Self::CustomSpring(spring)
    }
}
