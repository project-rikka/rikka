#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Intensity {
    Light,
    Moderate,
    Heavy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Condition {
    Clear,
    MainlyClear,
    PartlyCloudy,
    Overcast,
    Fog,
    RimeFog,
    Drizzle(Intensity),
    FreezingDrizzle(Intensity),
    Rain(Intensity),
    FreezingRain(Intensity),
    Snow(Intensity),
    SnowGrains,
    RainShowers(Intensity),
    SnowShowers(Intensity),
    Thunderstorm,
    ThunderstormWithHail(Intensity),
    Unknown,
}

impl Condition {
    #[must_use]
    pub fn from_wmo(code: u8) -> Condition {
        use Intensity::{Heavy, Light, Moderate};
        match code {
            0 => Condition::Clear,
            1 => Condition::MainlyClear,
            2 => Condition::PartlyCloudy,
            3 => Condition::Overcast,
            45 => Condition::Fog,
            48 => Condition::RimeFog,
            51 => Condition::Drizzle(Light),
            53 => Condition::Drizzle(Moderate),
            55 => Condition::Drizzle(Heavy),
            56 => Condition::FreezingDrizzle(Light),
            57 => Condition::FreezingDrizzle(Heavy),
            61 => Condition::Rain(Light),
            63 => Condition::Rain(Moderate),
            65 => Condition::Rain(Heavy),
            66 => Condition::FreezingRain(Light),
            67 => Condition::FreezingRain(Heavy),
            71 => Condition::Snow(Light),
            73 => Condition::Snow(Moderate),
            75 => Condition::Snow(Heavy),
            77 => Condition::SnowGrains,
            80 => Condition::RainShowers(Light),
            81 => Condition::RainShowers(Moderate),
            82 => Condition::RainShowers(Heavy),
            85 => Condition::SnowShowers(Light),
            86 => Condition::SnowShowers(Heavy),
            95 => Condition::Thunderstorm,
            96 => Condition::ThunderstormWithHail(Light),
            99 => Condition::ThunderstormWithHail(Heavy),
            _ => Condition::Unknown,
        }
    }

    #[must_use]
    pub fn is_wet(self) -> bool {
        matches!(
            self,
            Condition::Drizzle(_)
                | Condition::FreezingDrizzle(_)
                | Condition::Rain(_)
                | Condition::FreezingRain(_)
                | Condition::RainShowers(_)
                | Condition::Thunderstorm
                | Condition::ThunderstormWithHail(_)
        )
    }

    #[must_use]
    pub fn is_snowy(self) -> bool {
        matches!(
            self,
            Condition::Snow(_) | Condition::SnowGrains | Condition::SnowShowers(_)
        )
    }

    #[must_use]
    pub fn is_stormy(self) -> bool {
        matches!(
            self,
            Condition::Thunderstorm | Condition::ThunderstormWithHail(_)
        )
    }
}
