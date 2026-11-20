use std::ops::RangeInclusive;

use animate_motion::{Automation, Intensity};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tuning {
    Secondary,
    Size,
    Snap,
    Jiggle,
    Torso,
    Head,
    Arms,
    Legs,
    Bounce,
}

impl Tuning {
    pub const STYLE: [Self; 3] = [Self::Size, Self::Snap, Self::Jiggle];
    pub const REGIONS: [Self; 5] = [
        Self::Torso,
        Self::Head,
        Self::Arms,
        Self::Legs,
        Self::Bounce,
    ];

    pub fn value(self, automation: &Automation) -> f32 {
        let intensity = &automation.intensity;
        match self {
            Self::Secondary => automation.secondary,
            Self::Jiggle => automation.jiggle,
            Self::Size => intensity.size,
            Self::Snap => intensity.snap,
            Self::Torso => intensity.torso,
            Self::Head => intensity.head,
            Self::Arms => intensity.arms,
            Self::Legs => intensity.legs,
            Self::Bounce => intensity.bounce,
        }
    }

    pub fn set(self, automation: &mut Automation, value: f32) {
        let value = value.clamp(*self.range().start(), *self.range().end());
        let intensity = &mut automation.intensity;
        match self {
            Self::Secondary => automation.secondary = value,
            Self::Jiggle => automation.jiggle = value,
            Self::Size => intensity.size = value,
            Self::Snap => intensity.snap = value,
            Self::Torso => intensity.torso = value,
            Self::Head => intensity.head = value,
            Self::Arms => intensity.arms = value,
            Self::Legs => intensity.legs = value,
            Self::Bounce => intensity.bounce = value,
        }
    }

    pub fn range(self) -> RangeInclusive<f32> {
        match self {
            Self::Secondary | Self::Jiggle => 0.0..=1.0,
            _ => Intensity::RANGE,
        }
    }
}
