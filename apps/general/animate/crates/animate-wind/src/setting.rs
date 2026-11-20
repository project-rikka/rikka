use std::ops::RangeInclusive;

macro_rules! settings {
    ($($name:ident: $id:literal, $min:literal..=$max:literal;)+) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum WindSetting {
            $($name),+
        }

        impl WindSetting {
            pub const ALL: &'static [WindSetting] = &[$(WindSetting::$name),+];

            #[must_use]
            pub fn id(self) -> &'static str {
                match self {
                    $(WindSetting::$name => $id),+
                }
            }

            #[must_use]
            pub fn range(self) -> RangeInclusive<f32> {
                match self {
                    $(WindSetting::$name => $min..=$max),+
                }
            }
        }
    };
}

settings! {
    Yaw: "yaw", -180.0..=180.0;
    Pitch: "pitch", -90.0..=90.0;
    Speed: "speed", 0.0..=30.0;
    Turbulence: "turbulence", 0.0..=1.0;
    Gust: "gust", 0.0..=4.0;
}

impl WindSetting {
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|setting| setting.id() == id)
    }

    #[must_use]
    pub fn fits(self, value: f32) -> bool {
        self.range().contains(&value)
    }

    #[must_use]
    pub fn mix(self, from: f32, to: f32, t: f32) -> f32 {
        match self {
            Self::Yaw => wrap_degrees(from + wrap_degrees(to - from) * t),
            _ => from + (to - from) * t,
        }
    }
}

fn wrap_degrees(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}
