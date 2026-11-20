use std::collections::HashSet;

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::civil::{DAY, datetime_seconds};
use crate::date_value::DateValue;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::expansion::Expansion;
use crate::zones::Zones;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstanceSource {
    Series,
    Override(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub start: Timestamp,
    pub end: Timestamp,
    pub all_day: bool,
    pub local_start: DateValue,
    pub recurrence_id: DateValue,
    pub source: InstanceSource,
}

impl Instance {
    #[must_use]
    pub fn overlaps(&self, from: Timestamp, to: Timestamp) -> bool {
        self.start < to && (self.end > from || (self.end == self.start && self.start >= from))
    }
}

#[derive(Debug, Clone)]
pub struct Series<'a> {
    main: &'a Event,
    overrides: &'a [Event],
}

struct Frame<'z> {
    start: DateValue,
    tzid: Option<String>,
    zones: &'z Zones,
    floating: TimeZone,
    nominal_days: i64,
    exact_seconds: i64,
}

impl Frame<'_> {
    fn new<'z>(event: &Event, zones: &'z Zones, floating: &TimeZone) -> Result<Frame<'z>> {
        let start = event.start()?;
        let tzid = match &start {
            DateValue::Zoned { tzid, .. } => Some(tzid.clone()),
            DateValue::Utc(_) => Some("UTC".to_owned()),
            _ => None,
        };
        let (nominal_days, exact_seconds) = match (event.end()?, event.duration()?) {
            (Some(end), _) if start.is_date() => (
                ((end.local_seconds() - start.local_seconds()).div_euclid(DAY)).max(0),
                0,
            ),
            (Some(end), _) => {
                let begin = start.timestamp(zones, floating)?;
                let finish = end.timestamp(zones, floating)?;
                (0, (finish.as_second() - begin.as_second()).max(0))
            }
            (None, Some(duration)) => (duration.nominal_days(), duration.exact_seconds()),
            (None, None) if start.is_date() => (1, 0),
            (None, None) => (0, 0),
        };
        Ok(Frame {
            start,
            tzid,
            zones,
            floating: floating.clone(),
            nominal_days,
            exact_seconds,
        })
    }

    fn local(&self, value: &DateValue) -> Result<i64> {
        value.seconds_in_frame(&self.start, self.zones, &self.floating)
    }

    fn instant(&self, local: i64) -> Result<Timestamp> {
        self.zones
            .local_to_timestamp(local, self.tzid.as_deref(), &self.floating)
    }

    fn end(&self, local: i64) -> Result<Timestamp> {
        let base = self.instant(local + self.nominal_days * DAY)?;
        Ok(base.checked_add(jiff::SignedDuration::from_secs(self.exact_seconds))?)
    }

    fn value(&self, local: i64) -> Result<DateValue> {
        self.start
            .with_local_seconds(local)
            .ok_or(Error::OutOfRange)
    }

    fn local_at(&self, at: Timestamp) -> Result<i64> {
        Ok(match &self.tzid {
            Some(tzid) => datetime_seconds(self.zones.resolve(tzid)?.to_civil(at)),
            None => datetime_seconds(self.floating.to_datetime(at)),
        })
    }

    fn span(&self) -> i64 {
        (self.nominal_days * DAY + self.exact_seconds).max(0)
    }
}

impl<'a> Series<'a> {
    #[must_use]
    pub fn new(main: &'a Event, overrides: &'a [Event]) -> Series<'a> {
        Series { main, overrides }
    }

    #[must_use]
    pub fn single(main: &'a Event) -> Series<'a> {
        Series {
            main,
            overrides: &[],
        }
    }

    pub fn between(
        &self,
        from: Timestamp,
        to: Timestamp,
        zones: &Zones,
        floating: &TimeZone,
    ) -> Result<Vec<Instance>> {
        let mut found = Vec::new();
        self.walk(zones, floating, Some(from), |instance| {
            if instance.start >= to {
                return false;
            }
            if instance.overlaps(from, to) {
                found.push(instance);
            }
            true
        })?;
        for (index, event) in self.overrides.iter().enumerate() {
            if event.is_cancelled() {
                continue;
            }
            let instance = self.override_instance(index, event, zones, floating)?;
            if instance.overlaps(from, to) {
                found.push(instance);
            }
        }
        found.sort_by_key(|instance| (instance.start, instance.end));
        Ok(found)
    }

    pub fn first(&self, limit: usize, zones: &Zones, floating: &TimeZone) -> Result<Vec<Instance>> {
        let mut found = Vec::new();
        if limit == 0 {
            return Ok(found);
        }
        self.walk(zones, floating, None, |instance| {
            found.push(instance);
            found.len() < limit
        })?;
        for (index, event) in self.overrides.iter().enumerate() {
            if !event.is_cancelled() {
                found.push(self.override_instance(index, event, zones, floating)?);
            }
        }
        found.sort_by_key(|instance| (instance.start, instance.end));
        found.truncate(limit);
        Ok(found)
    }

    pub fn next_after(
        &self,
        at: Timestamp,
        zones: &Zones,
        floating: &TimeZone,
    ) -> Result<Option<Instance>> {
        let mut next = None;
        self.walk(zones, floating, Some(at), |instance| {
            if instance.start >= at {
                next = Some(instance);
                return false;
            }
            true
        })?;
        for (index, event) in self.overrides.iter().enumerate() {
            if event.is_cancelled() {
                continue;
            }
            let instance = self.override_instance(index, event, zones, floating)?;
            if instance.start >= at
                && next
                    .as_ref()
                    .is_none_or(|earlier: &Instance| instance.start < earlier.start)
            {
                next = Some(instance);
            }
        }
        Ok(next)
    }

    fn walk(
        &self,
        zones: &Zones,
        floating: &TimeZone,
        from: Option<Timestamp>,
        mut visit: impl FnMut(Instance) -> bool,
    ) -> Result<()> {
        let frame = Frame::new(self.main, zones, floating)?;
        let start = frame.start.local_seconds();

        let mut excluded_exact = HashSet::new();
        let mut excluded_days = HashSet::new();
        for date in self.main.exception_dates()? {
            let local = frame.local(&date)?;
            if date.is_date() && !frame.start.is_date() {
                excluded_days.insert(local.div_euclid(DAY));
            } else {
                excluded_exact.insert(local);
            }
        }
        for event in self.overrides {
            if let Some(id) = event.recurrence_id()? {
                excluded_exact.insert(frame.local(&id)?);
            }
        }

        let mut extra: Vec<i64> = self
            .main
            .recurrence_dates()?
            .iter()
            .map(|date| frame.local(date))
            .collect::<Result<_>>()?;
        extra.sort_unstable();
        extra.dedup();
        let mut extra = extra.into_iter().filter(|local| *local != start).peekable();

        let rule = self.main.rule()?;
        let until = match rule.as_ref().and_then(|rule| rule.until_value()) {
            Some(until) if until.is_date() && !frame.start.is_date() => {
                Some(until.local_seconds().div_euclid(DAY) * DAY + DAY - 1)
            }
            Some(until) => Some(frame.local(until)?),
            None => None,
        };
        let mut generated: Box<dyn Iterator<Item = i64>> = match &rule {
            Some(rule) => {
                let mut expansion = Expansion::new(rule, start, frame.start.is_date(), until);
                if let Some(from) = from {
                    expansion.seek(frame.local_at(from)? - frame.span() - 2 * DAY);
                }
                Box::new(expansion)
            }
            None => Box::new(std::iter::once(start)),
        };
        let mut pending_rule = generated.next();

        loop {
            let local = match (pending_rule, extra.peek().copied()) {
                (Some(ruled), Some(added)) if added < ruled => {
                    extra.next();
                    added
                }
                (Some(ruled), Some(added)) if added == ruled => {
                    extra.next();
                    pending_rule = generated.next();
                    ruled
                }
                (Some(ruled), _) => {
                    pending_rule = generated.next();
                    ruled
                }
                (None, Some(added)) => {
                    extra.next();
                    added
                }
                (None, None) => return Ok(()),
            };
            if excluded_exact.contains(&local) || excluded_days.contains(&local.div_euclid(DAY)) {
                continue;
            }
            let value = frame.value(local)?;
            let instance = Instance {
                start: frame.instant(local)?,
                end: frame.end(local)?,
                all_day: frame.start.is_date(),
                recurrence_id: value.clone(),
                local_start: value,
                source: InstanceSource::Series,
            };
            if !visit(instance) {
                return Ok(());
            }
        }
    }

    fn override_instance(
        &self,
        index: usize,
        event: &Event,
        zones: &Zones,
        floating: &TimeZone,
    ) -> Result<Instance> {
        let frame = Frame::new(event, zones, floating)?;
        let local = frame.start.local_seconds();
        Ok(Instance {
            start: frame.instant(local)?,
            end: frame.end(local)?,
            all_day: frame.start.is_date(),
            local_start: frame.start.clone(),
            recurrence_id: event
                .recurrence_id()?
                .unwrap_or_else(|| frame.start.clone()),
            source: InstanceSource::Override(index),
        })
    }
}
