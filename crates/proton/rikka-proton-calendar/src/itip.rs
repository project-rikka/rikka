use jiff::Timestamp;
use rikka_proton_contentline::ContentLine;

use crate::component::Component;
use crate::date_value::DateValue;
use crate::error::{Error, Result};
use crate::event::{Event, EventStatus, VEVENT};
use crate::icalendar::{ICalendar, Method};
use crate::party::{Attendee, ParticipationStatus};

const KEEP_FOR_REPLY_AND_CANCEL: [&str; 10] = [
    "UID",
    "DTSTART",
    "COMMENT",
    "DTEND",
    "SEQUENCE",
    "RECURRENCE-ID",
    "ORGANIZER",
    "RRULE",
    "LOCATION",
    "SUMMARY",
];
const KEEP_FOR_REPLY: [&str; 2] = ["X-PM-PROTON-REPLY", "EXDATE"];
const KEEP_FOR_CANCEL: [&str; 1] = ["X-PM-SHARED-EVENT-ID"];
const DROP_FOR_REQUEST: [&str; 3] = ["X-PM-PROTON-REPLY", "COLOR", "DTSTAMP"];
const PARTSTAT: &str = "PARTSTAT";
const SUMMARY: &str = "SUMMARY";
const RRULE: &str = "RRULE";

#[must_use]
pub fn request_calendar(
    event: &Event,
    prod_id: &str,
    timezones: Vec<Component>,
    timestamp: Timestamp,
) -> ICalendar {
    let mut invite = event.clone();
    let component = invite.component_mut();
    component.remove_components("VALARM");
    for name in DROP_FOR_REQUEST {
        component.remove(name);
    }
    if component.property(SUMMARY).is_none() {
        component.set_text(SUMMARY, "");
    }
    invite.set_stamp(timestamp);
    invite.remove_redundant_end();
    itip_calendar(Method::Request, invite, prod_id, timezones)
}

pub fn reply_calendar(
    event: &Event,
    attendee_email: &str,
    status: ParticipationStatus,
    prod_id: &str,
    timezones: Vec<Component>,
    timestamp: Timestamp,
) -> Result<ICalendar> {
    let attendee = event
        .attendee(attendee_email)
        .ok_or_else(|| Error::NotInvited(attendee_email.to_owned()))?;
    let mut line = ContentLine::new("ATTENDEE", attendee.line().value());
    line.set_parameter(rikka_proton_contentline::Parameter::new(
        PARTSTAT,
        status.as_str(),
    ));

    let mut invite = minimal_event(event, &KEEP_FOR_REPLY, timestamp)?;
    invite.component_mut().push(line);
    if invite.is_override() {
        invite.component_mut().remove(RRULE);
    }
    Ok(itip_calendar(Method::Reply, invite, prod_id, timezones))
}

pub fn cancel_calendar(
    event: &Event,
    attendees: &[Attendee],
    prod_id: &str,
    timezones: Vec<Component>,
    timestamp: Timestamp,
) -> Result<ICalendar> {
    let mut invite = minimal_event(event, &KEEP_FOR_CANCEL, timestamp)?;
    for attendee in attendees {
        invite
            .component_mut()
            .push(ContentLine::new("ATTENDEE", attendee.line().value()));
    }
    invite.set_status(EventStatus::Cancelled);
    Ok(itip_calendar(Method::Cancel, invite, prod_id, timezones))
}

fn minimal_event(event: &Event, extra: &[&str], timestamp: Timestamp) -> Result<Event> {
    let mut component = Component::new(VEVENT);
    for name in KEEP_FOR_REPLY_AND_CANCEL.iter().chain(extra) {
        for line in event.component().all(name) {
            component.push(line.clone());
        }
    }
    let mut invite = Event::from_component(component)?;
    invite.set_stamp(timestamp);
    invite.remove_redundant_end();
    Ok(invite)
}

fn itip_calendar(
    method: Method,
    event: Event,
    prod_id: &str,
    timezones: Vec<Component>,
) -> ICalendar {
    let mut calendar = ICalendar::new(prod_id).with_method(&method);
    for timezone in timezones {
        calendar.push_timezone(timezone);
    }
    calendar.push_event(event);
    calendar
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Itip {
    method: Method,
    calendar: ICalendar,
    events: Vec<Event>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Freshness {
    Newer,
    Same,
    Outdated,
}

impl Itip {
    pub fn parse(text: &str) -> Result<Itip> {
        let calendar = ICalendar::parse(text)?;
        let method = calendar.method().unwrap_or(Method::Publish);
        let events = calendar.events()?;
        if events.is_empty() {
            return Err(Error::NoEvents);
        }
        if events.iter().any(|event| event.uid().is_none()) {
            return Err(Error::MissingUid);
        }
        Ok(Itip {
            method,
            calendar,
            events,
        })
    }

    #[must_use]
    pub fn method(&self) -> &Method {
        &self.method
    }

    #[must_use]
    pub fn calendar(&self) -> &ICalendar {
        &self.calendar
    }

    #[must_use]
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    #[must_use]
    pub fn main_event(&self) -> &Event {
        self.events
            .iter()
            .find(|event| !event.is_override())
            .unwrap_or(&self.events[0])
    }

    #[must_use]
    pub fn uid(&self) -> String {
        self.main_event().uid().unwrap_or_default()
    }

    #[must_use]
    pub fn is_from_organizer(&self) -> bool {
        matches!(
            self.method,
            Method::Request
                | Method::Cancel
                | Method::Add
                | Method::DeclineCounter
                | Method::Publish
        )
    }

    #[must_use]
    pub fn invited_attendee(&self, own_emails: &[String]) -> Option<Attendee> {
        let attendees = self.main_event().attendees();
        own_emails.iter().find_map(|email| {
            attendees
                .iter()
                .find(|attendee| attendee.email().eq_ignore_ascii_case(email.trim()))
                .cloned()
        })
    }

    #[must_use]
    pub fn replying_attendee(&self) -> Option<Attendee> {
        match self.method {
            Method::Reply => self.main_event().attendees().into_iter().next(),
            _ => None,
        }
    }

    pub fn freshness(&self, current: &Event) -> Result<Freshness> {
        let incoming = self.main_event();
        let (theirs, ours) = (incoming.sequence(), current.sequence());
        if theirs != ours {
            return Ok(if theirs > ours {
                Freshness::Newer
            } else {
                Freshness::Outdated
            });
        }
        let stamp = |event: &Event| -> Result<Option<i64>> {
            Ok(event.stamp()?.map(|stamp| stamp.local_seconds()))
        };
        Ok(match (stamp(incoming)?, stamp(current)?) {
            (Some(theirs), Some(ours)) if theirs > ours => Freshness::Newer,
            (Some(theirs), Some(ours)) if theirs < ours => Freshness::Outdated,
            _ => Freshness::Same,
        })
    }

    pub fn apply_reply(&self, organizer_copy: &mut Event) -> Result<bool> {
        if self.method != Method::Reply {
            return Err(Error::UnexpectedMethod(self.method.as_str().to_owned()));
        }
        let Some(replying) = self.replying_attendee() else {
            return Ok(false);
        };
        if self.main_event().sequence() < organizer_copy.sequence() {
            return Ok(false);
        }
        let mut attendees = organizer_copy.attendees();
        let Some(attendee) = attendees
            .iter_mut()
            .find(|attendee| attendee.email().eq_ignore_ascii_case(&replying.email()))
        else {
            return Ok(false);
        };
        if attendee.status() == replying.status() {
            return Ok(false);
        }
        attendee.set_status(replying.status());
        organizer_copy.set_attendees(attendees);
        Ok(true)
    }

    pub fn cancels(&self, current: &Event) -> Result<bool> {
        if self.method != Method::Cancel {
            return Ok(false);
        }
        Ok(self.main_event().uid() == current.uid()
            && self.freshness(current)? != Freshness::Outdated)
    }

    pub fn recurrence_id(&self) -> Result<Option<DateValue>> {
        self.main_event().recurrence_id()
    }
}
