use rikka_proton_contentline::ContentLine;

use crate::component::Component;
use crate::error::{Error, Result};
use crate::event::{Event, VEVENT};
use crate::party::Attendee;

const SHARED_SIGNED: [&str; 10] = [
    "UID",
    "DTSTAMP",
    "DTSTART",
    "DTEND",
    "RECURRENCE-ID",
    "RRULE",
    "EXDATE",
    "ORGANIZER",
    "SEQUENCE",
    "X-PM-CONFERENCE-ID",
];
const SHARED_ENCRYPTED: [&str; 7] = [
    "UID",
    "DTSTAMP",
    "CREATED",
    "DESCRIPTION",
    "SUMMARY",
    "LOCATION",
    "X-PM-CONFERENCE-URL",
];
const CALENDAR_SIGNED: [&str; 5] = ["UID", "DTSTAMP", "EXDATE", "STATUS", "TRANSP"];
const CALENDAR_ENCRYPTED: [&str; 3] = ["UID", "DTSTAMP", "COMMENT"];
const REQUIRED: [&str; 2] = ["UID", "DTSTAMP"];
const ATTENDEE: &str = "ATTENDEE";
const COLOR: &str = "COLOR";
const PARTSTAT: &str = "PARTSTAT";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClearAttendee {
    pub(crate) token: String,
    pub(crate) status: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parts {
    pub(crate) shared_signed: String,
    pub(crate) shared_encrypted: String,
    pub(crate) calendar_signed: Option<String>,
    pub(crate) calendar_encrypted: Option<String>,
    pub(crate) attendees_encrypted: Option<String>,
    pub(crate) attendees_clear: Vec<ClearAttendee>,
}

pub(crate) fn split_event(event: &Event, prod_id: &str) -> Parts {
    let component = event.component();
    let is_assigned = |name: &str| {
        [
            &SHARED_SIGNED[..],
            &SHARED_ENCRYPTED[..],
            &CALENDAR_SIGNED[..],
            &CALENDAR_ENCRYPTED[..],
            &[ATTENDEE, COLOR][..],
        ]
        .iter()
        .any(|fields| fields.iter().any(|field| field.eq_ignore_ascii_case(name)))
    };

    let shared_signed = select_fields(component, &SHARED_SIGNED);
    let mut shared_encrypted = select_fields(component, &SHARED_ENCRYPTED);
    for line in component
        .properties()
        .iter()
        .filter(|line| !is_assigned(line.name()))
    {
        shared_encrypted.push(line.clone());
    }

    let calendar_signed = select_fields(component, &CALENDAR_SIGNED);
    let calendar_encrypted = select_fields(component, &CALENDAR_ENCRYPTED);

    let attendees: Vec<Attendee> = event.attendees();
    let attendees_clear = attendees
        .iter()
        .map(|attendee| ClearAttendee {
            token: attendee.token().unwrap_or_default().to_owned(),
            status: attendee.status().api_status(),
        })
        .collect();
    let attendees_encrypted = (!attendees.is_empty()).then(|| {
        let mut part = select_fields(component, &["UID"]);
        for attendee in &attendees {
            part.push(attendee.without_parameter(PARTSTAT).into_line());
        }
        wrap_vcalendar(&part, prod_id)
    });

    Parts {
        shared_signed: wrap_vcalendar(&shared_signed, prod_id),
        shared_encrypted: wrap_vcalendar(&shared_encrypted, prod_id),
        calendar_signed: has_optional_fields(&calendar_signed)
            .then(|| wrap_vcalendar(&calendar_signed, prod_id)),
        calendar_encrypted: has_optional_fields(&calendar_encrypted)
            .then(|| wrap_vcalendar(&calendar_encrypted, prod_id)),
        attendees_encrypted,
        attendees_clear,
    }
}

fn select_fields(component: &Component, fields: &[&str]) -> Component {
    let mut part = Component::new(VEVENT);
    for field in fields {
        for line in component.all(field) {
            part.push(line.clone());
        }
    }
    part
}

fn has_optional_fields(part: &Component) -> bool {
    part.properties()
        .iter()
        .any(|line| !REQUIRED.iter().any(|required| line.is(required)))
        || !part.components().is_empty()
}

pub(crate) fn wrap_vcalendar(part: &Component, prod_id: &str) -> String {
    let prod_id_line = match prod_id {
        "" => String::new(),
        prod_id => ContentLine::new("PRODID", prod_id).to_string(),
    };
    format!("BEGIN:VCALENDAR\r\nVERSION:2.0\r\n{prod_id_line}{part}END:VCALENDAR")
}

pub(crate) fn parse_vevents(text: &str) -> Result<Vec<Component>> {
    let mut events = Vec::new();
    for component in Component::parse_all(text)? {
        if component.is(VEVENT) {
            events.push(component);
        } else {
            events.extend(component.children(VEVENT).cloned());
        }
    }
    Ok(events)
}

pub(crate) fn merge_parts(parts: &[Component]) -> Result<Component> {
    if parts.is_empty() {
        return Err(Error::MissingProperty(VEVENT, "UID"));
    }
    let mut merged = Component::new(VEVENT);
    for part in parts {
        let mut names: Vec<&str> = part.properties().iter().map(ContentLine::name).collect();
        names.dedup();
        for name in names {
            merged.remove(name);
            for line in part.all(name) {
                merged.push(line.clone());
            }
        }
        for child in part.components() {
            merged.push_component(child.clone());
        }
    }
    Ok(merged)
}
