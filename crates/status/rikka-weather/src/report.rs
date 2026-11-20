use std::time::{Duration, SystemTime};

use jiff::civil::{Date, DateTime};
use jiff::tz::Offset;

use crate::air::Air;
use crate::condition::Condition;
use crate::place::Place;
use crate::quantity::{Distance, Precipitation, Pressure, Speed, Temperature};

const STALE: Duration = Duration::from_secs(3 * 3600);

pub const ATTRIBUTION: &str = "Weather data by Open-Meteo.com";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wind {
    pub speed: Speed,
    pub gusts: Option<Speed>,
    pub from_degrees: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Now {
    pub at: SystemTime,
    pub condition: Condition,
    pub daytime: bool,
    pub temperature: Temperature,
    pub feels_like: Option<Temperature>,
    pub dew_point: Option<Temperature>,
    pub humidity: Option<f32>,
    pub precipitation: Option<Precipitation>,
    pub cloud_cover: Option<f32>,
    pub pressure: Option<Pressure>,
    pub wind: Option<Wind>,
    pub visibility: Option<Distance>,
    pub uv_index: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hour {
    pub at: SystemTime,
    pub condition: Condition,
    pub daytime: bool,
    pub temperature: Temperature,
    pub feels_like: Option<Temperature>,
    pub precipitation_chance: Option<f32>,
    pub precipitation: Option<Precipitation>,
    pub wind: Option<Speed>,
    pub uv_index: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Day {
    pub date: Date,
    pub condition: Condition,
    pub high: Temperature,
    pub low: Temperature,
    pub sunrise: Option<SystemTime>,
    pub sunset: Option<SystemTime>,
    pub precipitation: Option<Precipitation>,
    pub precipitation_chance: Option<f32>,
    pub uv_index: Option<f32>,
    pub wind: Option<Wind>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub place: Place,
    pub fetched_at: SystemTime,
    pub timezone: String,
    pub offset: Offset,
    pub now: Now,
    pub hours: Vec<Hour>,
    pub days: Vec<Day>,
    pub air: Option<Air>,
}

impl Report {
    #[must_use]
    pub fn local(&self, at: SystemTime) -> DateTime {
        jiff::Timestamp::try_from(at)
            .map(|timestamp| self.offset.to_datetime(timestamp))
            .unwrap_or_default()
    }

    #[must_use]
    pub fn today(&self) -> Option<&Day> {
        self.day_of(SystemTime::now())
    }

    #[must_use]
    pub fn day_of(&self, at: SystemTime) -> Option<&Day> {
        let date = self.local(at).date();
        self.days.iter().find(|day| day.date == date)
    }

    #[must_use]
    pub fn hours_from(&self, at: SystemTime) -> &[Hour] {
        let hour = Duration::from_secs(3600);
        let first = self
            .hours
            .iter()
            .position(|entry| entry.at + hour > at)
            .unwrap_or(self.hours.len());
        &self.hours[first..]
    }

    #[must_use]
    pub fn is_stale(&self, now: SystemTime) -> bool {
        now.duration_since(self.fetched_at)
            .is_ok_and(|age| age > STALE)
    }
}
