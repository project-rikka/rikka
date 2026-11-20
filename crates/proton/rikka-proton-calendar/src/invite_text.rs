use jiff::civil::{Date, DateTime, Weekday};
use jiff::tz::{Offset, TimeZone};

use crate::civil::{DAY, date_from_seconds};
use crate::date_value::DateValue;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::icalendar::Method;
use crate::party::ParticipationStatus;
use crate::zones::Zones;

const DATE: &str = "{date}";
const TITLE: &str = "{title}";
const EMAIL: &str = "{email}";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InviteWording {
    pub invitation_on: String,
    pub invitation_starting: String,
    pub update_on: String,
    pub update_starting: String,
    pub cancellation_on: String,
    pub cancellation_starting: String,
    pub reply_prefix: String,
    pub untitled: String,
    pub title_label: String,
    pub time_label: String,
    pub location_label: String,
    pub description_label: String,
    pub note_label: String,
    pub all_day: String,
    pub removed: String,
    pub invited: String,
    pub occurrence_updated: String,
    pub series_updated: String,
    pub event_updated: String,
    pub what_changed: String,
    pub accepted: String,
    pub tentatively_accepted: String,
    pub declined: String,
    pub occurrence_cancelled: String,
    pub event_cancelled: String,
}

impl Default for InviteWording {
    fn default() -> InviteWording {
        let owned = |text: &str| text.to_owned();
        InviteWording {
            invitation_on: owned("Invitation for an event on {date}"),
            invitation_starting: owned("Invitation for an event starting on {date}"),
            update_on: owned("Update for an event on {date}"),
            update_starting: owned("Update for an event starting on {date}"),
            cancellation_on: owned("Cancellation of an event on {date}"),
            cancellation_starting: owned("Cancellation of an event starting on {date}"),
            reply_prefix: owned("Re:"),
            untitled: owned("(no title)"),
            title_label: owned("TITLE:"),
            time_label: owned("TIME:"),
            location_label: owned("LOCATION:"),
            description_label: owned("DESCRIPTION:"),
            note_label: owned("NOTE:"),
            all_day: owned("(all day)"),
            removed: owned("Removed"),
            invited: owned("You are invited to"),
            occurrence_updated: owned("This event occurrence was updated."),
            series_updated: owned("All events in this series were updated."),
            event_updated: owned("This event was updated."),
            what_changed: owned("Here's what changed:"),
            accepted: owned("{email} accepted your invitation to {title}"),
            tentatively_accepted: owned("{email} tentatively accepted your invitation to {title}"),
            declined: owned("{email} declined your invitation to {title}"),
            occurrence_cancelled: owned("This event occurrence was canceled."),
            event_cancelled: owned("{title} was canceled."),
        }
    }
}

pub trait DateWording {
    fn date(&self, date: Date) -> String;
    fn date_time(&self, local: DateTime) -> String;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnglishDates;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

impl DateWording for EnglishDates {
    fn date(&self, date: Date) -> String {
        let day = date.day();
        let suffix = match (day % 10, day % 100) {
            (_, 11..=13) => "th",
            (1, _) => "st",
            (2, _) => "nd",
            (3, _) => "rd",
            _ => "th",
        };
        format!(
            "{} {} {day}{suffix}, {}",
            weekday_name(date.weekday()),
            MONTHS[usize::try_from(date.month() - 1).unwrap_or_default()],
            date.year()
        )
    }

    fn date_time(&self, local: DateTime) -> String {
        let hour = local.hour();
        let (twelve, meridiem) = match hour {
            0 => (12, "AM"),
            1..=11 => (hour, "AM"),
            12 => (12, "PM"),
            _ => (hour - 12, "PM"),
        };
        format!(
            "{} at {twelve}:{:02} {meridiem}",
            self.date(local.date()),
            local.minute()
        )
    }
}

fn weekday_name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    }
}

#[must_use]
pub fn gmt_offset(offset: Offset) -> String {
    let seconds = offset.seconds();
    let sign = if seconds < 0 { '-' } else { '+' };
    let minutes = seconds.abs() / 60;
    match minutes % 60 {
        0 => format!("GMT{sign}{}", minutes / 60),
        rest => format!("GMT{sign}{}:{rest:02}", minutes / 60),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItipMail {
    pub subject: String,
    pub body: String,
}

#[must_use]
pub struct ItipMailBuilder<'a> {
    method: Method,
    event: &'a Event,
    previous: Option<&'a Event>,
    creating: bool,
    entire_series: bool,
    reply: Option<(&'a str, ParticipationStatus)>,
    wording: Option<&'a InviteWording>,
    dates: Option<&'a dyn DateWording>,
    zones: Option<&'a Zones>,
}

impl std::fmt::Debug for ItipMailBuilder<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ItipMailBuilder")
            .field("method", &self.method)
            .field("creating", &self.creating)
            .field("entire_series", &self.entire_series)
            .finish_non_exhaustive()
    }
}

struct DateInfo {
    start: String,
    end: Option<String>,
    all_day: bool,
    single_all_day: bool,
}

impl ItipMail {
    pub fn for_event(method: Method, event: &Event) -> ItipMailBuilder<'_> {
        ItipMailBuilder {
            method,
            event,
            previous: None,
            creating: false,
            entire_series: false,
            reply: None,
            wording: None,
            dates: None,
            zones: None,
        }
    }
}

impl<'a> ItipMailBuilder<'a> {
    pub fn previous(mut self, previous: &'a Event) -> Self {
        self.previous = Some(previous);
        self
    }

    pub fn creating(mut self) -> Self {
        self.creating = true;
        self
    }

    pub fn entire_series(mut self) -> Self {
        self.entire_series = true;
        self
    }

    pub fn reply_from(mut self, email: &'a str, status: ParticipationStatus) -> Self {
        self.reply = Some((email, status));
        self
    }

    pub fn wording(mut self, wording: &'a InviteWording) -> Self {
        self.wording = Some(wording);
        self
    }

    pub fn dates(mut self, dates: &'a dyn DateWording) -> Self {
        self.dates = Some(dates);
        self
    }

    pub fn zones(mut self, zones: &'a Zones) -> Self {
        self.zones = Some(zones);
        self
    }

    pub fn build(self) -> Result<ItipMail> {
        let defaults = InviteWording::default();
        let empty = Zones::new();
        let wording = self.wording.unwrap_or(&defaults);
        let dates = self.dates.unwrap_or(&EnglishDates);
        let zones = self.zones.unwrap_or(&empty);
        let composer = MailComposer {
            wording,
            dates,
            zones,
        };
        Ok(ItipMail {
            subject: composer.subject(&self)?,
            body: composer.body(&self)?,
        })
    }
}

struct Sections<'s> {
    title: &'s str,
    when: &'s str,
    location: &'s str,
    description: &'s str,
    comment: &'s str,
}

struct MailComposer<'a> {
    wording: &'a InviteWording,
    dates: &'a dyn DateWording,
    zones: &'a Zones,
}

impl MailComposer<'_> {
    fn subject(&self, request: &ItipMailBuilder<'_>) -> Result<String> {
        let info = self.date_info(&without_redundant_end(request.event))?;
        let wording = self.wording;
        let subject_for = |on: &str, starting: &str| {
            let template = if info.single_all_day { on } else { starting };
            template.replace(DATE, &info.start)
        };
        Ok(match &request.method {
            Method::Request => {
                let is_new_invitation = request.creating && !request.event.is_override();
                match is_new_invitation {
                    true => subject_for(&wording.invitation_on, &wording.invitation_starting),
                    false => subject_for(&wording.update_on, &wording.update_starting),
                }
            }
            Method::Cancel => subject_for(&wording.cancellation_on, &wording.cancellation_starting),
            Method::Reply => {
                let subject = subject_for(&wording.invitation_on, &wording.invitation_starting);
                let prefix = &wording.reply_prefix;
                match subject.to_lowercase().starts_with(&prefix.to_lowercase()) {
                    true => subject,
                    false => format!("{prefix} {subject}"),
                }
            }
            other => return Err(Error::UnexpectedMethod(other.as_str().to_owned())),
        })
    }

    fn body(&self, request: &ItipMailBuilder<'_>) -> Result<String> {
        let wording = self.wording;
        let event = without_redundant_end(request.event);
        let previous = request.previous.map(without_redundant_end);
        let title = self.title(&event);
        let when = self.when(&event)?;
        let labelled = |label: &str, value: Option<String>| -> String {
            value
                .filter(|value| !value.is_empty())
                .map(|value| format!("{label}\n{value}"))
                .unwrap_or_default()
        };
        let comment = first_comment(&event);
        let location_text = labelled(&wording.location_label, event.location());
        let description_text = labelled(&wording.description_label, event.description());
        let comment_text = labelled(&wording.note_label, comment.clone());
        let location_and_description = join_sections(&[&location_text, &description_text]);
        let details_with_comment = join_sections(&[&location_and_description, &comment_text]);
        let details = join_sections(&[&when, &details_with_comment]);
        let title_text = format!("{}\n{title}", wording.title_label);
        let sections = Sections {
            title: &title_text,
            when: &when,
            location: &location_text,
            description: &description_text,
            comment: &comment_text,
        };
        let updated = match &previous {
            Some(previous) => self.updated_details(&event, previous, &sections)?,
            None => String::new(),
        };
        let has_updated_comment = comment_changed(&event, previous.as_ref());
        let comment_status = if updated.is_empty() || !has_updated_comment {
            comment_text.clone()
        } else {
            String::new()
        };

        let body = BodyParts {
            title: &title,
            details: &details,
            updated: &updated,
            comment_status: &comment_status,
            has_updated_comment,
        };
        match &request.method {
            Method::Request => Ok(self.request_body(&event, request, &body)),
            Method::Cancel => Ok(self.cancel_body(&event, &body)),
            Method::Reply => self.reply_body(request, &body),
            other => Err(Error::UnexpectedMethod(other.as_str().to_owned())),
        }
    }

    fn request_body(
        &self,
        event: &Event,
        request: &ItipMailBuilder<'_>,
        body: &BodyParts<'_>,
    ) -> String {
        let wording = self.wording;
        let headline = if event.is_override() {
            &wording.occurrence_updated
        } else if request.entire_series {
            &wording.series_updated
        } else if request.creating {
            return format!("{} {}.\n\n{}", wording.invited, body.title, body.details);
        } else {
            &wording.event_updated
        };
        self.update_message(
            headline,
            body.updated,
            body.comment_status,
            body.has_updated_comment,
        )
    }

    fn cancel_body(&self, event: &Event, body: &BodyParts<'_>) -> String {
        let wording = self.wording;
        let intro = match event.is_override() {
            true => wording.occurrence_cancelled.clone(),
            false => wording.event_cancelled.replace(TITLE, body.title),
        };
        if !body.updated.is_empty() {
            format!(
                "{intro}\n\n{}\n\n{}{}",
                wording.what_changed,
                body.updated,
                comment_suffix(body.comment_status)
            )
        } else if !body.comment_status.is_empty() {
            format!(
                "{intro}\n\n{}\n{}",
                wording.what_changed, body.comment_status
            )
        } else {
            intro
        }
    }

    fn reply_body(&self, request: &ItipMailBuilder<'_>, body: &BodyParts<'_>) -> Result<String> {
        let wording = self.wording;
        let missing_status =
            || Error::UnexpectedMethod("REPLY without participation status".to_owned());
        let (email, status) = request.reply.ok_or_else(missing_status)?;
        let template = match status {
            ParticipationStatus::Accepted => &wording.accepted,
            ParticipationStatus::Tentative => &wording.tentatively_accepted,
            ParticipationStatus::Declined => &wording.declined,
            _ => return Err(missing_status()),
        };
        let response = template.replace(EMAIL, email).replace(TITLE, body.title);
        Ok(if !body.updated.is_empty() {
            format!(
                "{response}\n\n{}\n\n{}{}",
                wording.what_changed,
                body.updated,
                comment_suffix(body.comment_status)
            )
        } else if !body.comment_status.is_empty() {
            format!("{response}\n\n{}", body.comment_status)
        } else {
            response
        })
    }

    fn update_message(
        &self,
        intro: &str,
        updated: &str,
        comment_status: &str,
        comment_changed: bool,
    ) -> String {
        if !updated.trim().is_empty() || (comment_changed && !comment_status.is_empty()) {
            return format!(
                "{intro}\n\n{}\n\n{updated}{}",
                self.wording.what_changed,
                comment_suffix(comment_status)
            );
        }
        if !comment_status.is_empty() {
            return format!("{intro}\n\n{comment_status}");
        }
        intro.to_owned()
    }

    fn updated_details(
        &self,
        event: &Event,
        previous: &Event,
        sections: &Sections<'_>,
    ) -> Result<String> {
        let wording = self.wording;
        let is_same_text = |left: Option<String>, right: Option<String>| {
            left.unwrap_or_default().trim() == right.unwrap_or_default().trim()
        };
        let mut updated = String::new();
        if !is_same_text(event.summary(), previous.summary()) {
            updated = append_field(
                &updated,
                sections.title,
                &wording.title_label,
                &wording.removed,
            );
        }
        if times_changed(event, previous, self.zones)? {
            updated = append_field(
                &updated,
                sections.when,
                &wording.time_label,
                &wording.removed,
            );
        }
        if !is_same_text(event.location(), previous.location()) {
            updated = append_field(
                &updated,
                sections.location,
                &wording.location_label,
                &wording.removed,
            );
        }
        if !is_same_text(event.description(), previous.description()) {
            updated = append_field(
                &updated,
                sections.description,
                &wording.description_label,
                &wording.removed,
            );
        }
        if !is_same_text(first_comment(event), first_comment(previous)) {
            updated = append_field(
                &updated,
                sections.comment,
                &wording.note_label,
                &wording.removed,
            );
        }
        Ok(updated)
    }

    fn title(&self, event: &Event) -> String {
        event
            .summary()
            .map(|summary| summary.trim().to_owned())
            .filter(|summary| !summary.is_empty())
            .unwrap_or_else(|| self.wording.untitled.clone())
    }

    fn when(&self, event: &Event) -> Result<String> {
        let info = self.date_info(event)?;
        let label = &self.wording.time_label;
        Ok(match (&info.end, info.all_day, info.single_all_day) {
            (_, true, true) | (None, true, _) => {
                format!("{label}\n{} {}", info.start, self.wording.all_day)
            }
            (Some(end), _, _) => format!("{label}\n{} - {end}", info.start),
            (None, false, _) => format!("{label}\n{}", info.start),
        })
    }

    fn date_info(&self, event: &Event) -> Result<DateInfo> {
        let start = event.start()?;
        let end = event.end()?;
        if start.is_date() {
            let single = match &end {
                None => true,
                Some(end) => {
                    let span = end.local_seconds() - start.local_seconds();
                    span == DAY || span == 0
                }
            };
            let last = end
                .as_ref()
                .and_then(|end| date_from_seconds(end.local_seconds() - DAY));
            return Ok(DateInfo {
                start: self.dates.date(start.date()),
                end: last.map(|last| self.dates.date(last)),
                all_day: true,
                single_all_day: single,
            });
        }
        Ok(DateInfo {
            start: self.format_timed(&start)?,
            end: end.as_ref().map(|end| self.format_timed(end)).transpose()?,
            all_day: false,
            single_all_day: false,
        })
    }

    fn format_timed(&self, value: &DateValue) -> Result<String> {
        let timestamp = value.timestamp(self.zones, &TimeZone::UTC)?;
        let offset = match value.tzid() {
            Some(tzid) => self.zones.resolve(tzid)?.offset_at(timestamp),
            None => Offset::UTC,
        };
        Ok(format!(
            "{} ({})",
            self.dates.date_time(value.local()),
            gmt_offset(offset)
        ))
    }
}

fn append_field(updated: &str, text: &str, label: &str, removed: &str) -> String {
    let removed_text = format!("{label}\n{removed}");
    match (updated.is_empty(), text.is_empty()) {
        (true, true) => removed_text,
        (true, false) => text.to_owned(),
        (false, true) => format!("{updated}\n\n{removed_text}"),
        (false, false) => format!("{updated}\n\n{text}"),
    }
}

fn join_sections(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn comment_suffix(comment_status: &str) -> String {
    match comment_status.is_empty() {
        true => String::new(),
        false => format!("\n\n{comment_status}"),
    }
}

fn first_comment(event: &Event) -> Option<String> {
    event
        .component()
        .text("COMMENT")
        .filter(|comment| !comment.is_empty())
}

fn comment_changed(event: &Event, previous: Option<&Event>) -> bool {
    first_comment(event) != previous.and_then(first_comment)
}

fn times_changed(event: &Event, previous: &Event, zones: &Zones) -> Result<bool> {
    let instant = |value: &DateValue| value.timestamp(zones, &TimeZone::UTC);
    let start = instant(&event.start()?)? != instant(&previous.start()?)?;
    let end = instant(&event.end_or_derived()?)? != instant(&previous.end_or_derived()?)?;
    Ok(start || end)
}

fn without_redundant_end(event: &Event) -> Event {
    let mut trimmed = event.clone();
    trimmed.remove_redundant_end();
    trimmed
}

struct BodyParts<'a> {
    title: &'a str,
    details: &'a str,
    updated: &'a str,
    comment_status: &'a str,
    has_updated_comment: bool,
}
