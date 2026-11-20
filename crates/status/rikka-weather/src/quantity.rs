use crate::units::{DistanceUnit, PrecipitationUnit, PressureUnit, SpeedUnit, TemperatureUnit};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Temperature(pub f32);

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Speed(pub f32);

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Precipitation(pub f32);

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Pressure(pub f32);

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Distance(pub f32);

impl Temperature {
    #[must_use]
    pub fn celsius(self) -> f32 {
        self.0
    }

    #[must_use]
    pub fn in_unit(self, unit: TemperatureUnit) -> f32 {
        match unit {
            TemperatureUnit::Celsius => self.0,
            TemperatureUnit::Fahrenheit => self.0 * 9.0 / 5.0 + 32.0,
        }
    }
}

impl Speed {
    #[must_use]
    pub fn metres_per_second(self) -> f32 {
        self.0
    }

    #[must_use]
    pub fn in_unit(self, unit: SpeedUnit) -> f32 {
        match unit {
            SpeedUnit::MetresPerSecond => self.0,
            SpeedUnit::KilometresPerHour => self.0 * 3.6,
            SpeedUnit::MilesPerHour => self.0 * 3600.0 / 1609.344,
            SpeedUnit::Knots => self.0 * 3600.0 / 1852.0,
        }
    }

    #[must_use]
    pub fn beaufort(self) -> u8 {
        const LIMITS: [f32; 12] = [
            0.5, 1.6, 3.4, 5.5, 8.0, 10.8, 13.9, 17.2, 20.8, 24.5, 28.5, 32.7,
        ];
        LIMITS.iter().take_while(|limit| self.0 >= **limit).count() as u8
    }
}

impl Precipitation {
    #[must_use]
    pub fn millimetres(self) -> f32 {
        self.0
    }

    #[must_use]
    pub fn in_unit(self, unit: PrecipitationUnit) -> f32 {
        match unit {
            PrecipitationUnit::Millimetres => self.0,
            PrecipitationUnit::Inches => self.0 / 25.4,
        }
    }
}

impl Pressure {
    #[must_use]
    pub fn hectopascals(self) -> f32 {
        self.0
    }

    #[must_use]
    pub fn in_unit(self, unit: PressureUnit) -> f32 {
        match unit {
            PressureUnit::Hectopascals => self.0,
            PressureUnit::MillimetresOfMercury => self.0 * 0.750_061_7,
            PressureUnit::InchesOfMercury => self.0 * 0.029_529_98,
        }
    }
}

impl Distance {
    #[must_use]
    pub fn metres(self) -> f32 {
        self.0
    }

    #[must_use]
    pub fn in_unit(self, unit: DistanceUnit) -> f32 {
        match unit {
            DistanceUnit::Kilometres => self.0 / 1000.0,
            DistanceUnit::Miles => self.0 / 1609.344,
        }
    }
}
