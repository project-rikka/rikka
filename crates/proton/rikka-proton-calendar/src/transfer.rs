use std::collections::{BTreeMap, BTreeSet};

use futures_util::{StreamExt as _, TryStreamExt as _, stream};
use jiff::Timestamp;
use jiff::tz::TimeZone;
use rikka_proton_api::{Outcome, base64, offload, parallel_map_each};
use rikka_proton_pgp::{PublicKey, random_bytes};

use crate::calendar_event::CalendarEvent;
use crate::client::CalendarClient;
use crate::component::Component;
use crate::date_value::DateValue;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::icalendar::{ICalendar, Method};
use crate::opening::{OpeningKeys, decrypt_event};
use crate::rule::RuleEnd;
use crate::sync::{EventOptions, SyncedEvent, clock_timestamp};
use crate::unlocked::UnlockedCalendar;
use crate::zones::Zones;

pub const MAX_IMPORT_EVENTS: usize = 15_000;
const MAX_CONCURRENT_AUTHOR_LOOKUPS: usize = 8;
const DATE_PROPERTIES: [&str; 4] = ["DTSTART", "DTEND", "RECURRENCE-ID", "DTSTAMP"];
const DATE_LIST_PROPERTIES: [&str; 2] = ["EXDATE", "RDATE"];
const STRIPPED_PROPERTIES: [&str; 2] = ["ORGANIZER", "ATTENDEE"];
const CALENDAR_ZONE: &str = "X-WR-TIMEZONE";
const UID_DOMAIN: &str = "@proton.me";

#[derive(Debug, Clone)]
pub struct ImportFailure {
    pub uid: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Default)]
pub struct ImportReport {
    pub imported: Vec<CalendarEvent>,
    pub failed: Vec<ImportFailure>,
}

#[derive(Debug, Clone)]
pub struct PreparedImport {
    pub events: Vec<Event>,
    pub failed: Vec<ImportFailure>,
}

pub fn prepare_import(text: &str, floating_tzid: &str, now: Timestamp) -> Result<PreparedImport> {
    let calendar = ICalendar::parse(text)?;
    if let Some(method) = calendar
        .method()
        .filter(|method| *method != Method::Publish)
    {
        return Err(Error::UnsupportedMethod(method.as_str().to_owned()));
    }
    let zones = calendar.zones()?;
    let calendar_zone = calendar
        .component()
        .text(CALENDAR_ZONE)
        .and_then(|tzid| Zones::supported_tzid(&tzid));
    let floating = calendar_zone.as_deref().unwrap_or(floating_tzid).to_owned();
    let events = calendar.events()?;
    if events.is_empty() {
        return Err(Error::NoEvents);
    }
    if events.len() > MAX_IMPORT_EVENTS {
        return Err(Error::TooManyEvents(events.len(), MAX_IMPORT_EVENTS));
    }

    let mut prepared = Vec::with_capacity(events.len());
    let mut failed = Vec::new();
    for event in events {
        let uid = event.uid();
        match normalize_import_event(event, &zones, &floating, now) {
            Ok(event) => prepared.push(event),
            Err(error) => failed.push(ImportFailure {
                uid,
                reason: error.to_string(),
            }),
        }
    }
    prepared.sort_by_key(Event::is_override);
    Ok(PreparedImport {
        events: prepared,
        failed,
    })
}

fn normalize_import_event(
    event: Event,
    zones: &Zones,
    floating: &str,
    now: Timestamp,
) -> Result<Event> {
    let mut event = event;
    let start = event.start()?;
    if event.uid().is_none() {
        event.component_mut().set_text("UID", &proton_uid());
    }
    let component = event.component_mut();
    for name in STRIPPED_PROPERTIES {
        component.remove(name);
    }

    let floating_zone =
        Zones::lookup(floating).ok_or_else(|| Error::UnknownTimeZone(floating.to_owned()))?;
    let rewrite = |value: DateValue| -> Result<DateValue> {
        with_supported_zone(value, zones, floating, &floating_zone)
    };

    let mut lines = Vec::with_capacity(component.properties().len());
    for line in component.properties() {
        if DATE_PROPERTIES.iter().any(|name| line.is(name)) {
            lines.push(rewrite(DateValue::from_line(line)?)?.to_line(line.name()));
        } else if DATE_LIST_PROPERTIES.iter().any(|name| line.is(name)) {
            let values = DateValue::list_from_line(line)?
                .into_iter()
                .map(rewrite)
                .collect::<Result<Vec<_>>>()?;
            if let Some(rebuilt) = DateValue::list_to_line(line.name(), &values) {
                lines.push(rebuilt);
            }
        } else {
            lines.push(line.clone());
        }
    }
    *component.properties_mut() = lines;

    if let Some(mut rule) = event.rule()?
        && let RuleEnd::Until(until) = &rule.end
    {
        let until = match (start.is_date(), until) {
            (true, until) => DateValue::Date(until.date()),
            (false, DateValue::Date(date)) => {
                let last = date.to_datetime(jiff::civil::Time::constant(23, 59, 59, 0));
                let resolved = match start.tzid() {
                    Some(tzid) => zones.resolve(tzid)?.to_timestamp(last)?,
                    None => floating_zone.to_ambiguous_timestamp(last).compatible()?,
                };
                DateValue::utc(resolved)
            }
            (false, until) => {
                let framed = match until {
                    DateValue::Floating(local) => match start.tzid() {
                        Some(tzid) => DateValue::zoned(*local, tzid),
                        None => until.clone(),
                    },
                    other => other.clone(),
                };
                DateValue::utc(framed.timestamp(zones, &floating_zone)?)
            }
        };
        rule.end = RuleEnd::Until(until);
        event.set_rule(Some(&rule));
    }

    event.ensure_stamp(now);
    Ok(event)
}

fn with_supported_zone(
    value: DateValue,
    zones: &Zones,
    floating: &str,
    floating_zone: &TimeZone,
) -> Result<DateValue> {
    Ok(match value {
        DateValue::Date(_) | DateValue::Utc(_) => value,
        DateValue::Floating(local) => DateValue::zoned(local, floating),
        DateValue::Zoned { local, tzid } => match Zones::supported_tzid(&tzid) {
            Some(name) => DateValue::zoned(local, name),
            None => {
                let zoned = DateValue::Zoned { local, tzid };
                DateValue::utc(zoned.timestamp(zones, floating_zone)?)
            }
        },
    })
}

#[must_use]
pub fn proton_uid() -> String {
    let encoded = base64::encode(random_bytes::<21>())
        .replace('+', "-")
        .replace('/', "_")
        .trim_end_matches('=')
        .to_owned();
    format!("{encoded}{UID_DOMAIN}")
}

impl CalendarClient {
    pub async fn import(
        &self,
        calendar: &UnlockedCalendar,
        text: &str,
        floating_tzid: &str,
    ) -> Result<ImportReport> {
        let text = text.to_owned();
        let floating = floating_tzid.to_owned();
        let now = clock_timestamp(self.pgp())?;
        let prepared = offload(move || prepare_import(&text, &floating, now)).await?;

        let mut report = ImportReport {
            imported: Vec::new(),
            failed: prepared.failed,
        };
        let mut builder = self.sync(calendar).as_import();
        let mut uids = Vec::with_capacity(prepared.events.len());
        for event in prepared.events {
            uids.push(event.uid());
            builder = builder.create_with(
                event,
                EventOptions {
                    overwrite: true,
                    ..EventOptions::default()
                },
            );
        }
        let outcomes: Vec<Outcome<String, SyncedEvent>> = builder.execute().await?;
        for outcome in outcomes {
            match outcome.body().event() {
                Some(event) if outcome.is_ok() => report.imported.push(event.clone()),
                _ => report.failed.push(ImportFailure {
                    uid: uids.get(outcome.index()).cloned().flatten(),
                    reason: outcome.error().map_or_else(
                        || format!("API returned code {}", outcome.code().value()),
                        str::to_owned,
                    ),
                }),
            }
        }
        Ok(report)
    }

    pub async fn export(&self, calendar: &UnlockedCalendar) -> Result<ICalendar> {
        let records = self.all_events(calendar.calendar_id()).await?;
        let author_emails: BTreeSet<String> = records
            .iter()
            .map(|record| record.author().to_owned())
            .collect();
        let authors: BTreeMap<String, Vec<PublicKey>> = stream::iter(author_emails)
            .map(|email| async move {
                let keys = self.author_keys(&email).await?;
                Ok::<_, Error>((email, keys))
            })
            .buffer_unordered(MAX_CONCURRENT_AUTHOR_LOOKUPS)
            .try_collect()
            .await?;

        let pgp = *self.pgp();
        let unlocked = calendar.clone();
        let account = self.shared_keys();
        let email = calendar.member().email().to_owned();
        let decrypted_events: Vec<Event> = parallel_map_each(records, move |record| {
            let author = authors.get(record.author()).cloned().unwrap_or_default();
            let address = record
                .address_id()
                .and_then(|address| account.address_keys(address))
                .cloned();
            decrypt_event(
                &pgp,
                &record,
                OpeningKeys {
                    calendar: unlocked.keys(),
                    address: address.as_ref(),
                    author: &author,
                },
                Some(unlocked.settings()),
            )
            .map(|decrypted| publishable(decrypted.into_event(), &email))
        })
        .await
        .into_iter()
        .filter_map(|result| match result {
            Ok(event) => Some(event),
            Err(error) => {
                tracing::warn!(%error, "skipping event in export");
                None
            }
        })
        .collect();

        let mut tzids: Vec<String> = decrypted_events
            .iter()
            .flat_map(|event| {
                [event.start().ok(), event.end().ok().flatten()]
                    .into_iter()
                    .flatten()
                    .filter_map(|value| value.tzid().map(str::to_owned))
            })
            .collect();
        tzids.sort();
        tzids.dedup();
        let timezones: Vec<Component> = self.vtimezones(&tzids).await?;

        let mut exported = ICalendar::new(self.prod_id()).with_method(&Method::Publish);
        for timezone in timezones {
            exported.push_timezone(timezone);
        }
        for event in decrypted_events {
            exported.push_event(event);
        }
        Ok(exported)
    }
}

fn publishable(mut event: Event, email: &str) -> Event {
    let title = event.summary().unwrap_or_default();
    if event.component().property("SUMMARY").is_none() {
        event.set_summary("");
    }
    let alarms = event
        .alarms()
        .unwrap_or_else(|error| {
            tracing::warn!(%error, "dropping malformed alarms in export");
            Vec::new()
        })
        .into_iter()
        .map(|alarm| alarm.with_publish_fields(&title, email))
        .collect();
    event.set_alarms(alarms);
    event
}
