use rikka_ui::{Label, MaterialSymbol, tr};
use rikka_weather::{Condition, Temperature, TemperatureUnit};

#[must_use]
pub fn symbol(condition: Condition, daytime: bool) -> MaterialSymbol {
    match condition {
        Condition::Clear | Condition::MainlyClear if daytime => MaterialSymbol::CLEAR_DAY,
        Condition::Clear | Condition::MainlyClear => MaterialSymbol::CLEAR_NIGHT,
        Condition::PartlyCloudy if daytime => MaterialSymbol::PARTLY_CLOUDY_DAY,
        Condition::PartlyCloudy => MaterialSymbol::PARTLY_CLOUDY_NIGHT,
        Condition::Overcast | Condition::Unknown => MaterialSymbol::CLOUD,
        Condition::Fog | Condition::RimeFog => MaterialSymbol::FOGGY,
        Condition::Drizzle(_) | Condition::RainShowers(_) => MaterialSymbol::RAINY_LIGHT,
        Condition::Rain(_) => MaterialSymbol::RAINY,
        Condition::FreezingDrizzle(_) | Condition::FreezingRain(_) => MaterialSymbol::WEATHER_MIX,
        Condition::Snow(_) | Condition::SnowGrains | Condition::SnowShowers(_) => {
            MaterialSymbol::WEATHER_SNOWY
        }
        Condition::Thunderstorm => MaterialSymbol::THUNDERSTORM,
        Condition::ThunderstormWithHail(_) => MaterialSymbol::WEATHER_HAIL,
    }
}

#[must_use]
pub fn describe(condition: Condition) -> Label {
    let label = match condition {
        Condition::Clear => "clear",
        Condition::MainlyClear => "mainly-clear",
        Condition::PartlyCloudy => "partly-cloudy",
        Condition::Overcast => "overcast",
        Condition::Fog | Condition::RimeFog => "fog",
        Condition::Drizzle(_) => "drizzle",
        Condition::FreezingDrizzle(_) | Condition::FreezingRain(_) => "freezing-rain",
        Condition::Rain(_) => "rain",
        Condition::Snow(_) | Condition::SnowGrains => "snow",
        Condition::RainShowers(_) => "showers",
        Condition::SnowShowers(_) => "snow-showers",
        Condition::Thunderstorm => "thunderstorm",
        Condition::ThunderstormWithHail(_) => "hail",
        Condition::Unknown => "unknown",
    };
    tr!(&format!("shell.island.weather.{label}"))
}

#[must_use]
pub fn degrees(temperature: Temperature, unit: TemperatureUnit) -> String {
    format!("{}°", temperature.in_unit(unit).round() as i32)
}
