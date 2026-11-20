use jiff::Timestamp;
use rikka_proton_contentline::{ContentLine, escape_text};

use crate::alarm::{Alarm, VALARM};
use crate::civil::DAY;
use crate::component::Component;
use crate::date_value::DateValue;
use crate::duration::Duration;
use crate::error::{Error, Result};
use crate::party::{Attendee, Organizer};
use crate::rule::Rule;

pub(crate) const VEVENT: &str = "VEVENT";
const UID: &str = "UID";
const DTSTAMP: &str = "DTSTAMP";
const DTSTART: &str = "DTSTART";
const DTEND: &str = "DTEND";
const DURATION: &str = "DURATION";
const RRULE: &str = "RRULE";
const EXDATE: &str = "EXDATE";
const RDATE: &str = "RDATE";
const RECURRENCE_ID: &str = "RECURRENCE-ID";
const SUMMARY: &str = "SUMMARY";
const DESCRIPTION: &str = "DESCRIPTION";
const LOCATION: &str = "LOCATION";
const STATUS: &str = "STATUS";
const TRANSP: &str = "TRANSP";
const SEQUENCE: &str = "SEQUENCE";
const ORGANIZER: &str = "ORGANIZER";
const ATTENDEE: &str = "ATTENDEE";
const COLOR: &str = "COLOR";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventStatus {
    Tentative,
    Confirmed,
    Cancelled,
}

impl EventStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            EventStatus::Tentative => "TENTATIVE",
            EventStatus::Confirmed => "CONFIRMED",
            EventStatus::Cancelled => "CANCELLED",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Option<EventStatus> {
        Some(match text.trim().to_ascii_uppercase().as_str() {
            "TENTATIVE" => EventStatus::Tentative,
            "CONFIRMED" => EventStatus::Confirmed,
            "CANCELLED" => EventStatus::Cancelled,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    component: Component,
}

impl Event {
    #[must_use]
    pub fn new(uid: impl AsRef<str>, start: DateValue) -> Event {
        let mut component = Component::new(VEVENT);
        component.set_text(UID, uid.as_ref());
        component.set(start.to_line(DTSTART));
        Event { component }
    }

    pub fn from_component(component: Component) -> Result<Event> {
        if !component.is(VEVENT) {
            return Err(Error::WrongComponent(VEVENT, component.name().to_owned()));
        }
        Ok(Event { component })
    }

    #[must_use]
    pub fn component(&self) -> &Component {
        &self.component
    }

    pub fn component_mut(&mut self) -> &mut Component {
        &mut self.component
    }

    #[must_use]
    pub fn into_component(self) -> Component {
        self.component
    }

    #[must_use]
    pub fn uid(&self) -> Option<String> {
        self.component
            .text(UID)
            .filter(|uid| !uid.trim().is_empty())
    }

    pub fn start(&self) -> Result<DateValue> {
        DateValue::from_line(
            self.component
                .property(DTSTART)
                .ok_or(Error::MissingProperty(VEVENT, DTSTART))?,
        )
    }

    pub fn end(&self) -> Result<Option<DateValue>> {
        self.component
            .property(DTEND)
            .map(DateValue::from_line)
            .transpose()
    }

    pub fn duration(&self) -> Result<Option<Duration>> {
        self.component
            .property(DURATION)
            .map(|line| line.value().parse())
            .transpose()
    }

    pub fn stamp(&self) -> Result<Option<DateValue>> {
        self.component
            .property(DTSTAMP)
            .map(DateValue::from_line)
            .transpose()
    }

    pub fn rule(&self) -> Result<Option<Rule>> {
        self.component
            .property(RRULE)
            .map(|line| line.value().parse())
            .transpose()
    }

    pub fn exception_dates(&self) -> Result<Vec<DateValue>> {
        self.date_lists(EXDATE)
    }

    pub fn recurrence_dates(&self) -> Result<Vec<DateValue>> {
        self.date_lists(RDATE)
    }

    pub fn recurrence_id(&self) -> Result<Option<DateValue>> {
        self.component
            .property(RECURRENCE_ID)
            .map(DateValue::from_line)
            .transpose()
    }

    #[must_use]
    pub fn is_recurring(&self) -> bool {
        self.component.property(RRULE).is_some() || self.component.property(RDATE).is_some()
    }

    #[must_use]
    pub fn is_override(&self) -> bool {
        self.component.property(RECURRENCE_ID).is_some()
    }

    #[must_use]
    pub fn is_all_day(&self) -> bool {
        self.start().is_ok_and(|start| start.is_date())
    }

    #[must_use]
    pub fn summary(&self) -> Option<String> {
        self.component.text(SUMMARY)
    }

    #[must_use]
    pub fn description(&self) -> Option<String> {
        self.component.text(DESCRIPTION)
    }

    #[must_use]
    pub fn location(&self) -> Option<String> {
        self.component.text(LOCATION)
    }

    #[must_use]
    pub fn status(&self) -> Option<EventStatus> {
        self.component
            .property(STATUS)
            .and_then(|line| EventStatus::parse(line.value()))
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.status() == Some(EventStatus::Cancelled)
    }

    #[must_use]
    pub fn is_transparent(&self) -> bool {
        self.component
            .property(TRANSP)
            .is_some_and(|line| line.value().trim().eq_ignore_ascii_case("TRANSPARENT"))
    }

    #[must_use]
    pub fn sequence(&self) -> u32 {
        self.component
            .property(SEQUENCE)
            .and_then(|line| line.value().trim().parse::<i64>().ok())
            .map_or(0, |sequence| sequence.clamp(0, i64::from(u32::MAX)) as u32)
    }

    #[must_use]
    pub fn color(&self) -> Option<String> {
        self.component.text(COLOR)
    }

    #[must_use]
    pub fn organizer(&self) -> Option<Organizer> {
        self.component
            .property(ORGANIZER)
            .cloned()
            .map(Organizer::from_line)
    }

    #[must_use]
    pub fn attendees(&self) -> Vec<Attendee> {
        self.component
            .all(ATTENDEE)
            .cloned()
            .map(Attendee::from_line)
            .collect()
    }

    #[must_use]
    pub fn attendee(&self, email: &str) -> Option<Attendee> {
        let target = email.trim();
        self.attendees()
            .into_iter()
            .find(|attendee| attendee.email().eq_ignore_ascii_case(target))
    }

    pub fn alarms(&self) -> Result<Vec<Alarm>> {
        self.component
            .children(VALARM)
            .cloned()
            .map(Alarm::from_component)
            .collect()
    }

    pub fn end_or_derived(&self) -> Result<DateValue> {
        let start = self.start()?;
        if let Some(end) = self.end()? {
            return Ok(end);
        }
        let shift = match self.duration()? {
            Some(duration) => duration.nominal_days() * DAY + duration.exact_seconds(),
            None if start.is_date() => DAY,
            None => 0,
        };
        start
            .with_local_seconds(start.local_seconds() + shift)
            .ok_or(Error::OutOfRange)
    }

    #[must_use]
    pub fn with_summary(mut self, summary: &str) -> Event {
        self.set_summary(summary);
        self
    }

    #[must_use]
    pub fn with_description(mut self, description: &str) -> Event {
        self.component.set_text(DESCRIPTION, description);
        self
    }

    #[must_use]
    pub fn with_location(mut self, location: &str) -> Event {
        self.component.set_text(LOCATION, location);
        self
    }

    #[must_use]
    pub fn with_end(mut self, end: DateValue) -> Event {
        self.set_end(end);
        self
    }

    #[must_use]
    pub fn with_rule(mut self, rule: &Rule) -> Event {
        self.set_rule(Some(rule));
        self
    }

    #[must_use]
    pub fn with_status(mut self, status: EventStatus) -> Event {
        self.set_status(status);
        self
    }

    #[must_use]
    pub fn with_sequence(mut self, sequence: u32) -> Event {
        self.set_sequence(sequence);
        self
    }

    #[must_use]
    pub fn with_organizer(mut self, organizer: Organizer) -> Event {
        self.component.set(organizer.into_line());
        self
    }

    #[must_use]
    pub fn with_attendee(mut self, attendee: Attendee) -> Event {
        self.component.push(attendee.into_line());
        self
    }

    #[must_use]
    pub fn with_alarm(mut self, alarm: Alarm) -> Event {
        self.component.push_component(alarm.into_component());
        self
    }

    #[must_use]
    pub fn with_exception(mut self, date: DateValue) -> Event {
        self.component.push(date.to_line(EXDATE));
        self
    }

    #[must_use]
    pub fn with_recurrence_id(mut self, id: DateValue) -> Event {
        self.component.set(id.to_line(RECURRENCE_ID));
        self
    }

    #[must_use]
    pub fn with_stamp(mut self, timestamp: Timestamp) -> Event {
        self.set_stamp(timestamp);
        self
    }

    pub fn set_summary(&mut self, summary: &str) {
        self.component.set_text(SUMMARY, summary);
    }

    pub fn set_start(&mut self, start: DateValue) {
        self.component.set(start.to_line(DTSTART));
    }

    pub fn set_end(&mut self, end: DateValue) {
        self.component.remove(DURATION);
        self.component.set(end.to_line(DTEND));
    }

    pub fn set_rule(&mut self, rule: Option<&Rule>) {
        match rule {
            Some(rule) => self
                .component
                .set(ContentLine::new(RRULE, rule.to_string())),
            None => self.component.remove(RRULE),
        }
    }

    pub fn set_sequence(&mut self, sequence: u32) {
        self.component
            .set(ContentLine::new(SEQUENCE, sequence.to_string()));
    }

    pub fn set_stamp(&mut self, timestamp: Timestamp) {
        self.component
            .set(DateValue::utc(timestamp).to_line(DTSTAMP));
    }

    pub fn set_status(&mut self, status: EventStatus) {
        self.component
            .set(ContentLine::new(STATUS, status.as_str()));
    }

    pub fn set_color(&mut self, color: Option<&str>) {
        match color {
            Some(color) => self
                .component
                .set(ContentLine::new(COLOR, escape_text(color))),
            None => self.component.remove(COLOR),
        }
    }

    pub fn set_attendees(&mut self, attendees: Vec<Attendee>) {
        self.component.remove(ATTENDEE);
        for attendee in attendees {
            self.component.push(attendee.into_line());
        }
    }

    pub fn set_alarms(&mut self, alarms: Vec<Alarm>) {
        self.component.remove_components(VALARM);
        for alarm in alarms {
            self.component.push_component(alarm.into_component());
        }
    }

    pub fn set_exception_dates(&mut self, dates: &[DateValue]) {
        self.component.remove(EXDATE);
        for date in dates {
            self.component.push(date.to_line(EXDATE));
        }
    }

    pub fn ensure_stamp(&mut self, timestamp: Timestamp) {
        if self.component.property(DTSTAMP).is_none() {
            self.set_stamp(timestamp);
        }
    }

    pub(crate) fn remove_redundant_end(&mut self) {
        let (Ok(start), Ok(Some(end))) = (self.start(), self.end()) else {
            return;
        };
        let span = end.local_seconds() - start.local_seconds();
        let redundant = match start.is_date() {
            true => span.div_euclid(DAY) <= 1,
            false => start == end,
        };
        if redundant {
            self.component.remove(DTEND);
        }
    }

    fn date_lists(&self, name: &str) -> Result<Vec<DateValue>> {
        let mut dates = Vec::new();
        for line in self.component.all(name) {
            dates.extend(DateValue::list_from_line(line)?);
        }
        Ok(dates)
    }
}
