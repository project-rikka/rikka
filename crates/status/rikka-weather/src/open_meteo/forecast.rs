use std::time::{Duration, SystemTime};

use jiff::tz::Offset;
use serde::Deserialize;

use crate::condition::Condition;
use crate::place::Place;
use crate::quantity::{Distance, Precipitation, Pressure, Speed, Temperature};
use crate::report::{Day, Hour, Now, Report, Wind};

pub(crate) const URL: &str = "https://api.open-meteo.com/v1/forecast";

const CURRENT: &str = "temperature_2m,relative_humidity_2m,apparent_temperature,is_day,precipitation,weather_code,cloud_cover,pressure_msl,wind_speed_10m,wind_direction_10m,wind_gusts_10m,dew_point_2m,visibility,uv_index";
const HOURLY: &str = "temperature_2m,apparent_temperature,precipitation_probability,precipitation,weather_code,is_day,wind_speed_10m,uv_index";
const DAILY: &str = "weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,uv_index_max,precipitation_sum,precipitation_probability_max,wind_speed_10m_max,wind_gusts_10m_max,wind_direction_10m_dominant";

const HOURS: u32 = 48;
const DAYS: u32 = 10;

pub(crate) fn query(latitude: f64, longitude: f64) -> Vec<(&'static str, String)> {
    vec![
        ("latitude", format!("{latitude:.2}")),
        ("longitude", format!("{longitude:.2}")),
        ("current", CURRENT.to_owned()),
        ("hourly", HOURLY.to_owned()),
        ("daily", DAILY.to_owned()),
        ("timezone", "auto".to_owned()),
        ("timeformat", "unixtime".to_owned()),
        ("wind_speed_unit", "ms".to_owned()),
        ("forecast_hours", HOURS.to_string()),
        ("forecast_days", DAYS.to_string()),
    ]
}

#[derive(Debug, Deserialize)]
pub(crate) struct Forecast {
    utc_offset_seconds: i32,
    #[serde(default)]
    timezone: String,
    current: Option<Current>,
    #[serde(default)]
    hourly: Hourly,
    #[serde(default)]
    daily: Daily,
}

#[derive(Debug, Deserialize)]
struct Current {
    time: i64,
    temperature_2m: Option<f32>,
    relative_humidity_2m: Option<f32>,
    apparent_temperature: Option<f32>,
    is_day: Option<f32>,
    precipitation: Option<f32>,
    weather_code: Option<f32>,
    cloud_cover: Option<f32>,
    pressure_msl: Option<f32>,
    wind_speed_10m: Option<f32>,
    wind_direction_10m: Option<f32>,
    wind_gusts_10m: Option<f32>,
    dew_point_2m: Option<f32>,
    visibility: Option<f32>,
    uv_index: Option<f32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Hourly {
    time: Vec<i64>,
    temperature_2m: Vec<Option<f32>>,
    apparent_temperature: Vec<Option<f32>>,
    precipitation_probability: Vec<Option<f32>>,
    precipitation: Vec<Option<f32>>,
    weather_code: Vec<Option<f32>>,
    is_day: Vec<Option<f32>>,
    wind_speed_10m: Vec<Option<f32>>,
    uv_index: Vec<Option<f32>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Daily {
    time: Vec<i64>,
    weather_code: Vec<Option<f32>>,
    temperature_2m_max: Vec<Option<f32>>,
    temperature_2m_min: Vec<Option<f32>>,
    sunrise: Vec<Option<i64>>,
    sunset: Vec<Option<i64>>,
    uv_index_max: Vec<Option<f32>>,
    precipitation_sum: Vec<Option<f32>>,
    precipitation_probability_max: Vec<Option<f32>>,
    wind_speed_10m_max: Vec<Option<f32>>,
    wind_gusts_10m_max: Vec<Option<f32>>,
    wind_direction_10m_dominant: Vec<Option<f32>>,
}

fn at<T: Copy>(column: &[Option<T>], index: usize) -> Option<T> {
    column.get(index).copied().flatten()
}

pub(crate) fn instant(seconds: i64) -> SystemTime {
    match u64::try_from(seconds) {
        Ok(after) => SystemTime::UNIX_EPOCH + Duration::from_secs(after),
        Err(_) => SystemTime::UNIX_EPOCH,
    }
}

fn condition(code: Option<f32>) -> Condition {
    code.and_then(|code| u8::try_from(code.round() as i64).ok())
        .map_or(Condition::Unknown, Condition::from_wmo)
}

fn fraction(percent: Option<f32>) -> Option<f32> {
    percent.map(|percent| (percent / 100.0).clamp(0.0, 1.0))
}

impl Forecast {
    pub(crate) fn into_report(self, place: Place, fetched_at: SystemTime) -> Option<Report> {
        let offset = Offset::from_seconds(self.utc_offset_seconds).ok()?;
        let current = self.current?;
        let now = Now {
            at: instant(current.time),
            condition: condition(current.weather_code),
            daytime: current.is_day.is_none_or(|day| day >= 0.5),
            temperature: Temperature(current.temperature_2m?),
            feels_like: current.apparent_temperature.map(Temperature),
            dew_point: current.dew_point_2m.map(Temperature),
            humidity: fraction(current.relative_humidity_2m),
            precipitation: current.precipitation.map(Precipitation),
            cloud_cover: fraction(current.cloud_cover),
            pressure: current.pressure_msl.map(Pressure),
            wind: current.wind_speed_10m.map(|speed| Wind {
                speed: Speed(speed),
                gusts: current.wind_gusts_10m.map(Speed),
                from_degrees: current.wind_direction_10m,
            }),
            visibility: current.visibility.map(Distance),
            uv_index: current.uv_index,
        };
        let hourly = &self.hourly;
        let hours = hourly
            .time
            .iter()
            .enumerate()
            .filter_map(|(index, time)| {
                Some(Hour {
                    at: instant(*time),
                    condition: condition(at(&hourly.weather_code, index)),
                    daytime: at(&hourly.is_day, index).is_none_or(|day| day >= 0.5),
                    temperature: Temperature(at(&hourly.temperature_2m, index)?),
                    feels_like: at(&hourly.apparent_temperature, index).map(Temperature),
                    precipitation_chance: fraction(at(&hourly.precipitation_probability, index)),
                    precipitation: at(&hourly.precipitation, index).map(Precipitation),
                    wind: at(&hourly.wind_speed_10m, index).map(Speed),
                    uv_index: at(&hourly.uv_index, index),
                })
            })
            .collect();
        let daily = &self.daily;
        let days = daily
            .time
            .iter()
            .enumerate()
            .filter_map(|(index, time)| {
                let midnight = jiff::Timestamp::from_second(*time).ok()?;
                Some(Day {
                    date: offset.to_datetime(midnight).date(),
                    condition: condition(at(&daily.weather_code, index)),
                    high: Temperature(at(&daily.temperature_2m_max, index)?),
                    low: Temperature(at(&daily.temperature_2m_min, index)?),
                    sunrise: at(&daily.sunrise, index).map(instant),
                    sunset: at(&daily.sunset, index).map(instant),
                    precipitation: at(&daily.precipitation_sum, index).map(Precipitation),
                    precipitation_chance: fraction(at(&daily.precipitation_probability_max, index)),
                    uv_index: at(&daily.uv_index_max, index),
                    wind: at(&daily.wind_speed_10m_max, index).map(|speed| Wind {
                        speed: Speed(speed),
                        gusts: at(&daily.wind_gusts_10m_max, index).map(Speed),
                        from_degrees: at(&daily.wind_direction_10m_dominant, index),
                    }),
                })
            })
            .collect();
        Some(Report {
            place,
            fetched_at,
            timezone: self.timezone,
            offset,
            now,
            hours,
            days,
            air: None,
        })
    }
}
