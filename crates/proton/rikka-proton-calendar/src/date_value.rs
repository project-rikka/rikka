use std::fmt;

use jiff::Timestamp;
use jiff::civil::{Date, DateTime};
use jiff::tz::TimeZone;
use rikka_proton_contentline::{ContentLine, Parameter};

use crate::civil::{DAY, date_from_seconds, date_seconds, datetime_from_seconds, datetime_seconds};
use crate::error::{Error, Result};
use crate::zones::Zones;

const VALUE: &str = "VALUE";
const TZID: &str = "TZID";
const DATE: &str = "DATE";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DateValue {
    Date(Date),
    Floating(DateTime),
    Utc(DateTime),
    Zoned { local: DateTime, tzid: String },
}

impl DateValue {
    pub fn from_line(line: &ContentLine) -> Result<DateValue> {
        let mut values = DateValue::list_from_line(line)?;
        match values.len() {
            0 => Err(Error::InvalidValue(
                line.name().to_owned(),
                line.value().to_owned(),
            )),
            _ => Ok(values.remove(0)),
        }
    }

    pub fn list_from_line(line: &ContentLine) -> Result<Vec<DateValue>> {
        let date = line
            .parameter_value(VALUE)
            .is_some_and(|value| value.eq_ignore_ascii_case(DATE));
        let tzid = line.parameter_value(TZID);
        line.value()
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| {
                let start = part.split('/').next().unwrap_or(part);
                let parsed = match date {
                    true => DateValue::parse_date(start),
                    false => DateValue::parse(start, tzid),
                };
                parsed.ok_or_else(|| {
                    Error::InvalidValue(line.name().to_owned(), line.value().to_owned())
                })
            })
            .collect()
    }

    #[must_use]
    pub fn parse_date(text: &str) -> Option<DateValue> {
        parse_date(text.trim()).map(DateValue::Date)
    }

    #[must_use]
    pub fn parse(text: &str, tzid: Option<&str>) -> Option<DateValue> {
        let text = text.trim();
        if text.len() == 8 && !text.contains('T') {
            return DateValue::parse_date(text);
        }
        let (body, utc) = match text.strip_suffix(['Z', 'z']) {
            Some(body) => (body, true),
            None => (text, false),
        };
        let local = parse_datetime(body)?;
        Some(match (utc, tzid) {
            (true, _) => DateValue::Utc(local),
            (false, Some(tzid)) if !tzid.trim().is_empty() => DateValue::Zoned {
                local,
                tzid: tzid.trim().to_owned(),
            },
            (false, _) => DateValue::Floating(local),
        })
    }

    #[must_use]
    pub fn utc(timestamp: Timestamp) -> DateValue {
        DateValue::Utc(TimeZone::UTC.to_datetime(timestamp))
    }

    #[must_use]
    pub fn zoned(local: DateTime, tzid: impl Into<String>) -> DateValue {
        DateValue::Zoned {
            local,
            tzid: tzid.into(),
        }
    }

    #[must_use]
    pub fn to_line(&self, name: &str) -> ContentLine {
        let line = ContentLine::new(name, self.to_string());
        match self {
            DateValue::Date(_) => line.with_parameter(Parameter::new(VALUE, DATE)),
            DateValue::Zoned { tzid, .. } => {
                line.with_parameter(Parameter::new(TZID, tzid.clone()))
            }
            DateValue::Floating(_) | DateValue::Utc(_) => line,
        }
    }

    #[must_use]
    pub fn list_to_line(name: &str, values: &[DateValue]) -> Option<ContentLine> {
        let first = values.first()?;
        let mut line = first.to_line(name);
        line.set_value(
            values
                .iter()
                .map(DateValue::to_string)
                .collect::<Vec<_>>()
                .join(","),
        );
        Some(line)
    }

    #[must_use]
    pub fn is_date(&self) -> bool {
        matches!(self, DateValue::Date(_))
    }

    #[must_use]
    pub fn tzid(&self) -> Option<&str> {
        match self {
            DateValue::Zoned { tzid, .. } => Some(tzid),
            _ => None,
        }
    }

    #[must_use]
    pub fn local(&self) -> DateTime {
        match self {
            DateValue::Date(date) => date.to_datetime(jiff::civil::Time::midnight()),
            DateValue::Floating(local) | DateValue::Utc(local) | DateValue::Zoned { local, .. } => {
                *local
            }
        }
    }

    #[must_use]
    pub fn date(&self) -> Date {
        self.local().date()
    }

    pub fn timestamp(&self, zones: &Zones, floating: &TimeZone) -> Result<Timestamp> {
        match self {
            DateValue::Utc(local) => Ok(Timestamp::from_second(datetime_seconds(*local))?),
            DateValue::Zoned { local, tzid } => zones.resolve(tzid)?.to_timestamp(*local),
            DateValue::Floating(local) => {
                Ok(floating.to_ambiguous_timestamp(*local).compatible()?)
            }
            DateValue::Date(date) => Ok(floating
                .to_ambiguous_timestamp(date.to_datetime(jiff::civil::Time::midnight()))
                .compatible()?),
        }
    }

    pub(crate) fn local_seconds(&self) -> i64 {
        match self {
            DateValue::Date(date) => date_seconds(*date),
            _ => datetime_seconds(self.local()),
        }
    }

    pub(crate) fn with_local_seconds(&self, seconds: i64) -> Option<DateValue> {
        Some(match self {
            DateValue::Date(_) => DateValue::Date(date_from_seconds(seconds)?),
            DateValue::Floating(_) => DateValue::Floating(datetime_from_seconds(seconds)?),
            DateValue::Utc(_) => DateValue::Utc(datetime_from_seconds(seconds)?),
            DateValue::Zoned { tzid, .. } => DateValue::Zoned {
                local: datetime_from_seconds(seconds)?,
                tzid: tzid.clone(),
            },
        })
    }

    pub(crate) fn seconds_in_frame(
        &self,
        frame: &DateValue,
        zones: &Zones,
        floating: &TimeZone,
    ) -> Result<i64> {
        if frame.is_date() || self.is_date() {
            return Ok(self.local_seconds().div_euclid(DAY) * DAY
                + if frame.is_date() {
                    0
                } else {
                    self.local_seconds().rem_euclid(DAY)
                });
        }
        let timestamp = self.timestamp(zones, floating)?;
        Ok(match frame {
            DateValue::Utc(_) => timestamp.as_second(),
            DateValue::Zoned { tzid, .. } => {
                datetime_seconds(zones.resolve(tzid)?.to_civil(timestamp))
            }
            _ => datetime_seconds(floating.to_datetime(timestamp)),
        })
    }
}

impl fmt::Display for DateValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DateValue::Date(date) => write_date(formatter, *date),
            DateValue::Floating(local) | DateValue::Zoned { local, .. } => {
                write_datetime(formatter, *local)
            }
            DateValue::Utc(local) => {
                write_datetime(formatter, *local)?;
                formatter.write_str("Z")
            }
        }
    }
}

fn parse_date(text: &str) -> Option<Date> {
    let digits: String = text.chars().filter(|character| *character != '-').collect();
    if digits.len() != 8 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Date::new(
        digits[0..4].parse().ok()?,
        digits[4..6].parse().ok()?,
        digits[6..8].parse().ok()?,
    )
    .ok()
}

fn parse_datetime(text: &str) -> Option<DateTime> {
    let (date, time) = text.split_once(['T', 't'])?;
    let date = parse_date(date)?;
    let digits: String = time.chars().filter(|character| *character != ':').collect();
    if !(digits.len() == 4 || digits.len() == 6)
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let hour: i8 = digits[0..2].parse().ok()?;
    let minute: i8 = digits[2..4].parse().ok()?;
    let second: i8 = match digits.len() {
        6 => digits[4..6].parse::<i8>().ok()?.min(59),
        _ => 0,
    };
    let time = jiff::civil::Time::new(hour, minute, second, 0).ok()?;
    Some(date.to_datetime(time))
}

fn write_date(formatter: &mut fmt::Formatter<'_>, date: Date) -> fmt::Result {
    write!(
        formatter,
        "{:04}{:02}{:02}",
        date.year(),
        date.month(),
        date.day()
    )
}

fn write_datetime(formatter: &mut fmt::Formatter<'_>, local: DateTime) -> fmt::Result {
    write_date(formatter, local.date())?;
    write!(
        formatter,
        "T{:02}{:02}{:02}",
        local.hour(),
        local.minute(),
        local.second()
    )
}
