use std::ops::RangeInclusive;

macro_rules! settings {
    ($($name:ident: $id:literal, $min:literal..=$max:literal;)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum PropSetting {
            $($name),+
        }

        impl PropSetting {
            pub const ALL: &'static [PropSetting] = &[$(PropSetting::$name),+];

            #[must_use]
            pub fn id(self) -> &'static str {
                match self {
                    $(PropSetting::$name => $id),+
                }
            }

            #[must_use]
            pub fn range(self) -> RangeInclusive<f32> {
                match self {
                    $(PropSetting::$name => $min..=$max),+
                }
            }
        }
    };
}

settings! {
    X: "x", -1000.0..=1000.0;
    Y: "y", -1000.0..=1000.0;
    Z: "z", -1000.0..=1000.0;
    Pitch: "pitch", -360.0..=360.0;
    Yaw: "yaw", -360.0..=360.0;
    Roll: "roll", -360.0..=360.0;
    Scale: "scale", 0.01..=100.0;
    Visible: "visible", 0.0..=1.0;
}

impl PropSetting {
    pub const POSITION: [PropSetting; 3] = [Self::X, Self::Y, Self::Z];
    pub const ROTATION: [PropSetting; 3] = [Self::Pitch, Self::Yaw, Self::Roll];

    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|setting| setting.id() == id)
    }

    #[must_use]
    pub fn fits(self, value: f32) -> bool {
        match self {
            Self::Visible => value == 0.0 || value == 1.0,
            _ => self.range().contains(&value),
        }
    }

    #[must_use]
    pub fn is_held(self) -> bool {
        self == Self::Visible
    }

    #[must_use]
    pub fn mix(self, from: f32, to: f32, t: f32) -> f32 {
        match self {
            Self::Visible => from,
            _ => from + (to - from) * t,
        }
    }
}
