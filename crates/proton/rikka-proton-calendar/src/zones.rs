use std::collections::HashMap;
use std::sync::Arc;

use jiff::Timestamp;
use jiff::civil::DateTime;
use jiff::tz::{Offset, TimeZone};

use crate::civil::{DAY, datetime_from_seconds, datetime_seconds};
use crate::component::Component;
use crate::date_value::DateValue;
use crate::error::{Error, Result};
use crate::expansion::Expansion;
use crate::rule::Rule;
use crate::zone_names::iana_zone_for;

const VTIMEZONE: &str = "VTIMEZONE";
const TZID: &str = "TZID";
const STANDARD: &str = "STANDARD";
const DAYLIGHT: &str = "DAYLIGHT";
const DTSTART: &str = "DTSTART";
const RRULE: &str = "RRULE";
const RDATE: &str = "RDATE";
const OFFSET_FROM: &str = "TZOFFSETFROM";
const OFFSET_TO: &str = "TZOFFSETTO";
const YEAR: i64 = 366 * DAY;
const UTC_NAMES: [&str; 4] = ["UTC", "GMT", "Z", "Etc/UTC"];

#[derive(Debug, Clone)]
pub enum Zone {
    Named(TimeZone),
    Defined(Arc<DefinedZone>),
}

impl Zone {
    pub fn to_timestamp(&self, local: DateTime) -> Result<Timestamp> {
        match self {
            Zone::Named(zone) => Ok(zone.to_ambiguous_timestamp(local).compatible()?),
            Zone::Defined(zone) => zone.to_timestamp(local),
        }
    }

    #[must_use]
    pub fn to_civil(&self, timestamp: Timestamp) -> DateTime {
        match self {
            Zone::Named(zone) => zone.to_datetime(timestamp),
            Zone::Defined(zone) => zone.offset_at(timestamp.as_second()).to_datetime(timestamp),
        }
    }

    #[must_use]
    pub fn offset_at(&self, timestamp: Timestamp) -> Offset {
        match self {
            Zone::Named(zone) => zone.to_offset(timestamp),
            Zone::Defined(zone) => zone.offset_at(timestamp.as_second()),
        }
    }

    #[must_use]
    pub fn iana_name(&self) -> Option<&str> {
        match self {
            Zone::Named(zone) => zone.iana_name(),
            Zone::Defined(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
struct Observance {
    start: DateValue,
    rule: Option<Rule>,
    dates: Vec<i64>,
    offset_from: Offset,
    offset_to: Offset,
}

impl Observance {
    fn last_onset_until(&self, utc: i64) -> Option<i64> {
        let offset_from = i64::from(self.offset_from.seconds());
        let local_limit = utc + offset_from;
        let start = self.start.local_seconds();
        let mut latest = (start <= local_limit).then_some(start);

        if let Some(rule) = &self.rule {
            let until = rule.until_value().map(|until| match until {
                DateValue::Utc(_) => until.local_seconds() + offset_from,
                DateValue::Date(_) => until.local_seconds() + DAY - 1,
                _ => until.local_seconds(),
            });
            let mut onsets = Expansion::new(rule, start, false, until);
            let horizon = until.map_or(local_limit, |until| until.min(local_limit));
            onsets.seek(horizon - 2 * YEAR);
            for onset in onsets {
                if onset > local_limit {
                    break;
                }
                latest = Some(onset);
            }
        }
        for date in &self.dates {
            if *date <= local_limit && latest.is_none_or(|latest_date| *date > latest_date) {
                latest = Some(*date);
            }
        }

        latest.map(|local| local - offset_from)
    }
}

#[derive(Debug, Clone)]
pub struct DefinedZone {
    tzid: String,
    observances: Vec<Observance>,
}

impl DefinedZone {
    pub fn from_component(component: &Component) -> Result<DefinedZone> {
        let tzid = component
            .text(TZID)
            .ok_or(Error::MissingProperty(VTIMEZONE, TZID))?;
        let mut observances = Vec::new();

        for child in component
            .components()
            .iter()
            .filter(|child| child.is(STANDARD) || child.is(DAYLIGHT))
        {
            let start = DateValue::from_line(
                child
                    .property(DTSTART)
                    .ok_or(Error::MissingProperty(STANDARD, DTSTART))?,
            )?;
            let offset_from = parse_offset(child, OFFSET_FROM)?;
            let offset_to = parse_offset(child, OFFSET_TO)?;
            let rule = child
                .property(RRULE)
                .map(|line| line.value().parse())
                .transpose()?;
            let mut dates = Vec::new();
            for line in child.all(RDATE) {
                dates.extend(
                    DateValue::list_from_line(line)?
                        .iter()
                        .map(DateValue::local_seconds),
                );
            }
            observances.push(Observance {
                start,
                rule,
                dates,
                offset_from,
                offset_to,
            });
        }

        if observances.is_empty() {
            return Err(Error::MissingProperty(VTIMEZONE, STANDARD));
        }

        Ok(DefinedZone { tzid, observances })
    }

    #[must_use]
    pub fn tzid(&self) -> &str {
        &self.tzid
    }

    fn offset_at(&self, utc: i64) -> Offset {
        let current = self
            .observances
            .iter()
            .filter_map(|observance| Some((observance.last_onset_until(utc)?, observance)))
            .max_by_key(|(onset, _)| *onset);
        match current {
            Some((_, observance)) => observance.offset_to,
            None => self
                .observances
                .iter()
                .min_by_key(|observance| observance.start.local_seconds())
                .map_or(Offset::UTC, |observance| observance.offset_from),
        }
    }

    fn to_timestamp(&self, local: DateTime) -> Result<Timestamp> {
        let wall = datetime_seconds(local);
        let before = self.offset_at(wall - DAY).seconds();
        let after = self.offset_at(wall + DAY).seconds();
        let mut candidates: Vec<i64> = [before, after]
            .into_iter()
            .map(|offset| wall - i64::from(offset))
            .filter(|utc| i64::from(self.offset_at(*utc).seconds()) == wall - utc)
            .collect();
        candidates.sort_unstable();
        let utc = candidates
            .first()
            .copied()
            .unwrap_or(wall - i64::from(before));
        Ok(Timestamp::from_second(utc)?)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Zones {
    defined: HashMap<String, Arc<DefinedZone>>,
}

impl Zones {
    #[must_use]
    pub fn new() -> Zones {
        Zones::default()
    }

    pub fn define(&mut self, component: &Component) -> Result<()> {
        let zone = DefinedZone::from_component(component)?;
        self.defined.insert(zone.tzid.clone(), Arc::new(zone));
        Ok(())
    }

    pub fn define_all<'a>(
        &mut self,
        components: impl IntoIterator<Item = &'a Component>,
    ) -> Result<()> {
        for component in components {
            if component.is(VTIMEZONE) {
                self.define(component)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn lookup(tzid: &str) -> Option<TimeZone> {
        let trimmed = tzid.trim().trim_matches('"');
        if UTC_NAMES
            .iter()
            .any(|name| name.eq_ignore_ascii_case(trimmed))
        {
            return Some(TimeZone::UTC);
        }
        let mut candidate = trimmed;
        loop {
            if let Some(zone) = lookup(candidate) {
                return Some(zone);
            }
            match candidate.split_once('/') {
                Some((_, rest)) if !rest.is_empty() => candidate = rest,
                _ => return None,
            }
        }
    }

    #[must_use]
    pub fn is_known(tzid: &str) -> bool {
        Zones::lookup(tzid).is_some()
    }

    #[must_use]
    pub fn supported_tzid(tzid: &str) -> Option<String> {
        if let Some(zone) = Zones::lookup(tzid) {
            return Some(zone.iana_name().unwrap_or("UTC").to_owned());
        }
        iana_zone_for(tzid)
    }

    #[must_use]
    pub fn get(&self, tzid: &str) -> Option<Zone> {
        Zones::lookup(tzid)
            .or_else(|| iana_zone_for(tzid).and_then(|name| Zones::lookup(&name)))
            .map(Zone::Named)
            .or_else(|| self.defined.get(tzid).cloned().map(Zone::Defined))
    }

    pub fn resolve(&self, tzid: &str) -> Result<Zone> {
        self.get(tzid)
            .ok_or_else(|| Error::UnknownTimeZone(tzid.to_owned()))
    }

    pub fn to_local(&self, timestamp: Timestamp, tzid: &str) -> Result<DateTime> {
        Ok(self.resolve(tzid)?.to_civil(timestamp))
    }

    pub(crate) fn local_to_timestamp(
        &self,
        seconds: i64,
        tzid: Option<&str>,
        floating: &TimeZone,
    ) -> Result<Timestamp> {
        let local = datetime_from_seconds(seconds).ok_or(Error::OutOfRange)?;
        match tzid {
            Some(tzid) => self.resolve(tzid)?.to_timestamp(local),
            None => Ok(floating.to_ambiguous_timestamp(local).compatible()?),
        }
    }
}

fn lookup(name: &str) -> Option<TimeZone> {
    if let Ok(zone) = jiff::tz::db().get(name) {
        return Some(zone);
    }
    let (canonical, data) = jiff_tzdb::get(name)?;
    TimeZone::tzif(canonical, data).ok()
}

fn parse_offset(observance: &Component, name: &'static str) -> Result<Offset> {
    let text = observance
        .property(name)
        .ok_or(Error::MissingProperty(STANDARD, name))?
        .value()
        .trim();
    let invalid = || Error::InvalidValue(name.to_owned(), text.to_owned());
    let (sign, digits) = match text.as_bytes().first() {
        Some(b'+') => (1, &text[1..]),
        Some(b'-') => (-1, &text[1..]),
        _ => return Err(invalid()),
    };
    if !(digits.len() == 4 || digits.len() == 6)
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(invalid());
    }
    let hours: i32 = digits[0..2].parse().map_err(|_| invalid())?;
    let minutes: i32 = digits[2..4].parse().map_err(|_| invalid())?;
    let seconds: i32 = if digits.len() == 6 {
        digits[4..6].parse().map_err(|_| invalid())?
    } else {
        0
    };
    Offset::from_seconds(sign * (hours * 3600 + minutes * 60 + seconds)).map_err(|_| invalid())
}
