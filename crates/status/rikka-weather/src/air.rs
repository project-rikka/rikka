use std::time::SystemTime;

#[derive(Clone, Debug, PartialEq)]
pub struct Air {
    pub at: SystemTime,
    pub us_aqi: Option<u16>,
    pub european_aqi: Option<u16>,
    pub pm2_5: Option<f32>,
    pub pm10: Option<f32>,
    pub ozone: Option<f32>,
    pub nitrogen_dioxide: Option<f32>,
    pub sulphur_dioxide: Option<f32>,
    pub carbon_monoxide: Option<f32>,
    pub uv_index: Option<f32>,
    pub pollen: Option<Pollen>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pollen {
    pub alder: Option<f32>,
    pub birch: Option<f32>,
    pub grass: Option<f32>,
    pub mugwort: Option<f32>,
    pub olive: Option<f32>,
    pub ragweed: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AirQuality {
    Good,
    Moderate,
    UnhealthyForSensitive,
    Unhealthy,
    VeryUnhealthy,
    Hazardous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UvLevel {
    Low,
    Moderate,
    High,
    VeryHigh,
    Extreme,
}

impl Air {
    #[must_use]
    pub fn quality(&self) -> Option<AirQuality> {
        Some(AirQuality::of_us_aqi(self.us_aqi?))
    }
}

impl AirQuality {
    #[must_use]
    pub fn of_us_aqi(index: u16) -> AirQuality {
        match index {
            0..=50 => AirQuality::Good,
            51..=100 => AirQuality::Moderate,
            101..=150 => AirQuality::UnhealthyForSensitive,
            151..=200 => AirQuality::Unhealthy,
            201..=300 => AirQuality::VeryUnhealthy,
            _ => AirQuality::Hazardous,
        }
    }
}

impl UvLevel {
    #[must_use]
    pub fn from_index(index: f32) -> UvLevel {
        match index.round() as i32 {
            ..=2 => UvLevel::Low,
            3..=5 => UvLevel::Moderate,
            6..=7 => UvLevel::High,
            8..=10 => UvLevel::VeryHigh,
            _ => UvLevel::Extreme,
        }
    }
}

impl Pollen {
    pub(crate) fn is_empty(&self) -> bool {
        [
            self.alder,
            self.birch,
            self.grass,
            self.mugwort,
            self.olive,
            self.ragweed,
        ]
        .iter()
        .all(Option::is_none)
    }
}
