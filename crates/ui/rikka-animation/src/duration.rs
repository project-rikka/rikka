use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DurationToken {
    Short1,
    Short2,
    Short3,
    Short4,
    Medium1,
    Medium2,
    Medium3,
    Medium4,
    Long1,
    Long2,
    Long3,
    Long4,
    ExtraLong1,
    ExtraLong2,
    ExtraLong3,
    ExtraLong4,
}

impl DurationToken {
    pub const fn millis(self) -> u64 {
        match self {
            Self::Short1 => 50,
            Self::Short2 => 100,
            Self::Short3 => 150,
            Self::Short4 => 200,
            Self::Medium1 => 250,
            Self::Medium2 => 300,
            Self::Medium3 => 350,
            Self::Medium4 => 400,
            Self::Long1 => 450,
            Self::Long2 => 500,
            Self::Long3 => 550,
            Self::Long4 => 600,
            Self::ExtraLong1 => 700,
            Self::ExtraLong2 => 800,
            Self::ExtraLong3 => 900,
            Self::ExtraLong4 => 1000,
        }
    }

    pub const fn duration(self) -> Duration {
        Duration::from_millis(self.millis())
    }

    pub const fn seconds(self) -> f32 {
        self.millis() as f32 / 1000.0
    }
}

impl From<DurationToken> for Duration {
    fn from(token: DurationToken) -> Self {
        token.duration()
    }
}
