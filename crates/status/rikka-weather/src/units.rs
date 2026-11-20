use rikka_settings::session::weather::{
    CELSIUS, DISTANCE_UNIT, FAHRENHEIT, HECTOPASCALS, INCHES, INCHES_OF_MERCURY, KILOMETRES,
    KILOMETRES_PER_HOUR, KNOTS, METRES_PER_SECOND, MILES, MILES_PER_HOUR, MILLIMETRES,
    MILLIMETRES_OF_MERCURY, PRECIPITATION_UNIT, PRESSURE_UNIT, SPEED_UNIT, TEMPERATURE_UNIT,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpeedUnit {
    KilometresPerHour,
    MetresPerSecond,
    MilesPerHour,
    Knots,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PrecipitationUnit {
    Millimetres,
    Inches,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PressureUnit {
    Hectopascals,
    MillimetresOfMercury,
    InchesOfMercury,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DistanceUnit {
    Kilometres,
    Miles,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Units {
    pub temperature: TemperatureUnit,
    pub speed: SpeedUnit,
    pub precipitation: PrecipitationUnit,
    pub pressure: PressureUnit,
    pub distance: DistanceUnit,
}

const FAHRENHEIT_REGIONS: [&str; 8] = ["US", "LR", "MM", "BS", "BZ", "KY", "PW", "FM"];
const MILES_REGIONS: [&str; 4] = ["US", "GB", "LR", "MM"];
const INCHES_REGIONS: [&str; 3] = ["US", "LR", "MM"];

impl Units {
    #[must_use]
    pub fn metric() -> Units {
        Units {
            temperature: TemperatureUnit::Celsius,
            speed: SpeedUnit::KilometresPerHour,
            precipitation: PrecipitationUnit::Millimetres,
            pressure: PressureUnit::Hectopascals,
            distance: DistanceUnit::Kilometres,
        }
    }

    #[must_use]
    pub fn for_region(region: &str) -> Units {
        let region = region.to_ascii_uppercase();
        let within = |regions: &[&str]| regions.contains(&region.as_str());
        Units {
            temperature: match within(&FAHRENHEIT_REGIONS) {
                true => TemperatureUnit::Fahrenheit,
                false => TemperatureUnit::Celsius,
            },
            speed: match within(&MILES_REGIONS) {
                true => SpeedUnit::MilesPerHour,
                false => SpeedUnit::KilometresPerHour,
            },
            precipitation: match within(&INCHES_REGIONS) {
                true => PrecipitationUnit::Inches,
                false => PrecipitationUnit::Millimetres,
            },
            pressure: match within(&INCHES_REGIONS) {
                true => PressureUnit::InchesOfMercury,
                false => PressureUnit::Hectopascals,
            },
            distance: match within(&MILES_REGIONS) {
                true => DistanceUnit::Miles,
                false => DistanceUnit::Kilometres,
            },
        }
    }

    #[must_use]
    pub fn for_locale() -> Units {
        measurement_region()
            .or_else(|| rikka_i18n::current_locale().region().map(str::to_owned))
            .map(|region| Units::for_region(&region))
            .unwrap_or_else(Units::metric)
    }

    #[must_use]
    pub fn from_settings(read: impl Fn(&str) -> Option<String>) -> Units {
        let usual = Units::for_locale();
        let named = |key| read(key).unwrap_or_default();
        Units {
            temperature: TemperatureUnit::named(&named(TEMPERATURE_UNIT))
                .unwrap_or(usual.temperature),
            speed: SpeedUnit::named(&named(SPEED_UNIT)).unwrap_or(usual.speed),
            precipitation: PrecipitationUnit::named(&named(PRECIPITATION_UNIT))
                .unwrap_or(usual.precipitation),
            pressure: PressureUnit::named(&named(PRESSURE_UNIT)).unwrap_or(usual.pressure),
            distance: DistanceUnit::named(&named(DISTANCE_UNIT)).unwrap_or(usual.distance),
        }
    }
}

fn measurement_region() -> Option<String> {
    rikka_types::LocaleCategory::Measurement
        .locale()?
        .territory()
        .map(str::to_ascii_uppercase)
}

impl TemperatureUnit {
    #[must_use]
    pub fn named(name: &str) -> Option<TemperatureUnit> {
        match name {
            CELSIUS => Some(TemperatureUnit::Celsius),
            FAHRENHEIT => Some(TemperatureUnit::Fahrenheit),
            _ => None,
        }
    }

    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            TemperatureUnit::Celsius => "°C",
            TemperatureUnit::Fahrenheit => "°F",
        }
    }
}

impl SpeedUnit {
    #[must_use]
    pub fn named(name: &str) -> Option<SpeedUnit> {
        match name {
            KILOMETRES_PER_HOUR => Some(SpeedUnit::KilometresPerHour),
            METRES_PER_SECOND => Some(SpeedUnit::MetresPerSecond),
            MILES_PER_HOUR => Some(SpeedUnit::MilesPerHour),
            KNOTS => Some(SpeedUnit::Knots),
            _ => None,
        }
    }

    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            SpeedUnit::KilometresPerHour => "km/h",
            SpeedUnit::MetresPerSecond => "m/s",
            SpeedUnit::MilesPerHour => "mph",
            SpeedUnit::Knots => "kn",
        }
    }
}

impl PrecipitationUnit {
    #[must_use]
    pub fn named(name: &str) -> Option<PrecipitationUnit> {
        match name {
            MILLIMETRES => Some(PrecipitationUnit::Millimetres),
            INCHES => Some(PrecipitationUnit::Inches),
            _ => None,
        }
    }

    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            PrecipitationUnit::Millimetres => "mm",
            PrecipitationUnit::Inches => "in",
        }
    }
}

impl PressureUnit {
    #[must_use]
    pub fn named(name: &str) -> Option<PressureUnit> {
        match name {
            HECTOPASCALS => Some(PressureUnit::Hectopascals),
            MILLIMETRES_OF_MERCURY => Some(PressureUnit::MillimetresOfMercury),
            INCHES_OF_MERCURY => Some(PressureUnit::InchesOfMercury),
            _ => None,
        }
    }

    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            PressureUnit::Hectopascals => "hPa",
            PressureUnit::MillimetresOfMercury => "mmHg",
            PressureUnit::InchesOfMercury => "inHg",
        }
    }
}

impl DistanceUnit {
    #[must_use]
    pub fn named(name: &str) -> Option<DistanceUnit> {
        match name {
            KILOMETRES => Some(DistanceUnit::Kilometres),
            MILES => Some(DistanceUnit::Miles),
            _ => None,
        }
    }

    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            DistanceUnit::Kilometres => "km",
            DistanceUnit::Miles => "mi",
        }
    }
}
