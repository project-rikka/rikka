use std::collections::HashSet;

use futures_util::future::try_join_all;
use futures_util::{TryStreamExt, stream};
use jiff::Timestamp;
use rikka_proton_api::{Page, Paging, offload};
use serde::{Deserialize, Serialize};

use crate::calendar_event::{AttendeeRecord, CalendarEvent};
use crate::changes::{CalendarAlarm, ModelEvents};
use crate::client::{CALENDAR_V1, CalendarClient};
use crate::error::{Error, Result};
use crate::identifier::{AttendeeId, CalendarId, EventId, ModelEventId};
use crate::notification::Notification;
use crate::opening::{DecryptedEvent, OpeningKeys, decrypt_event};
use crate::party::ParticipationStatus;
use crate::unlocked::UnlockedCalendar;

const PAGE_SIZE: u32 = 100;
const WINDOW_KINDS: [i32; 4] = [0, 1, 2, 3];
const MAX_CONCURRENT_ATTENDEE_FETCHES: usize = 8;

#[derive(Deserialize)]
struct AttendeesResponse {
    #[serde(rename = "Attendees", default)]
    attendees: Vec<AttendeeRecord>,
    #[serde(rename = "MoreAttendees", default, with = "rikka_proton_api::flag")]
    more: bool,
}

#[derive(Serialize)]
struct StatusBody {
    #[serde(rename = "Status")]
    status: i32,
    #[serde(rename = "UpdateTime", with = "rikka_proton_api::unix_seconds")]
    update_time: Timestamp,
}

#[derive(Serialize)]
struct PersonalBody<'a> {
    #[serde(rename = "Notifications")]
    notifications: Option<&'a [Notification]>,
    #[serde(rename = "Color")]
    color: Option<&'a str>,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct EventQuery {
    client: CalendarClient,
    calendar: CalendarId,
    from: Timestamp,
    to: Timestamp,
    timezone: String,
    metadata_only: bool,
}

impl EventQuery {
    pub fn timezone(mut self, tzid: impl Into<String>) -> EventQuery {
        self.timezone = tzid.into();
        self
    }

    pub fn metadata_only(mut self) -> EventQuery {
        self.metadata_only = true;
        self
    }

    pub async fn list(self) -> Result<Vec<CalendarEvent>> {
        let windows = try_join_all(WINDOW_KINDS.map(|kind| self.window(kind))).await?;
        let mut visited = HashSet::new();
        let mut collected: Vec<CalendarEvent> = windows
            .into_iter()
            .flatten()
            .filter(|event| visited.insert(event.id().clone()))
            .collect();
        self.client.complete_all_attendees(&mut collected).await?;
        self.client.cache_events(&collected);
        Ok(collected)
    }

    async fn window(&self, kind: i32) -> Result<Vec<CalendarEvent>> {
        Ok(self
            .client
            .api()
            .get(format!("{CALENDAR_V1}/{}/events", self.calendar))
            .query(&[
                ("Start", self.from.as_second().to_string()),
                ("End", self.to.as_second().to_string()),
                ("Timezone", self.timezone.clone()),
                ("Type", kind.to_string()),
                ("MetaDataOnly", u8::from(self.metadata_only).to_string()),
            ])
            .pages(Paging::numbered(PAGE_SIZE))
            .field("Events")
            .collect(Page::new)
            .await?)
    }
}

impl CalendarClient {
    pub fn events_between(
        &self,
        calendar: &CalendarId,
        from: Timestamp,
        to: Timestamp,
    ) -> EventQuery {
        EventQuery {
            client: self.clone(),
            calendar: calendar.clone(),
            from,
            to,
            timezone: "UTC".to_owned(),
            metadata_only: false,
        }
    }

    pub async fn event(&self, calendar: &CalendarId, id: &EventId) -> Result<CalendarEvent> {
        let response: CalendarEvent = self
            .api()
            .get(format!("{CALENDAR_V1}/{calendar}/events/{id}"))
            .send_field("Event")
            .await?;
        let mut event = response;
        self.complete_attendees(&mut event).await?;
        self.cache_events(std::slice::from_ref(&event));
        Ok(event)
    }

    pub async fn events_by_uid(
        &self,
        uid: &str,
        recurrence_id: Option<i64>,
    ) -> Result<Vec<CalendarEvent>> {
        let mut request = self
            .api()
            .get(format!("{CALENDAR_V1}/events"))
            .query(&[("UID", uid)]);
        if let Some(recurrence_id) = recurrence_id {
            request = request.query(&[("RecurrenceID", recurrence_id)]);
        }
        let mut collected: Vec<CalendarEvent> = request
            .pages(Paging::numbered(PAGE_SIZE))
            .field("Events")
            .collect(Page::new)
            .await?;
        self.complete_all_attendees(&mut collected).await?;
        self.cache_events(&collected);
        Ok(collected)
    }

    pub async fn all_events(&self, calendar: &CalendarId) -> Result<Vec<CalendarEvent>> {
        let mut visited = HashSet::new();
        let pages: Vec<Option<CalendarEvent>> = self
            .api()
            .get(format!("{CALENDAR_V1}/{calendar}/events"))
            .pages(Paging::cursor("BeginID").page_size(PAGE_SIZE))
            .field("Events")
            .collect(|events: Vec<CalendarEvent>| {
                let unseen: Vec<Option<CalendarEvent>> = events
                    .into_iter()
                    .map(|event| visited.insert(event.id().clone()).then_some(event))
                    .collect();
                let next = unseen
                    .iter()
                    .flatten()
                    .next_back()
                    .map(|event| event.id().clone());
                Page::new(unseen).next(next)
            })
            .await?;
        let mut collected: Vec<CalendarEvent> = pages.into_iter().flatten().collect();
        self.complete_all_attendees(&mut collected).await?;
        self.cache_events(&collected);
        Ok(collected)
    }

    async fn complete_all_attendees(&self, events: &mut [CalendarEvent]) -> Result<()> {
        stream::iter(events.iter_mut().map(Ok::<_, Error>))
            .try_for_each_concurrent(MAX_CONCURRENT_ATTENDEE_FETCHES, |event| {
                self.complete_attendees(event)
            })
            .await
    }

    pub(crate) async fn complete_attendees(&self, event: &mut CalendarEvent) -> Result<()> {
        let mut page = 1u32;
        while event.has_more_attendees() {
            let response: AttendeesResponse = self
                .api()
                .get(format!(
                    "{CALENDAR_V1}/{}/events/{}/attendees",
                    event.calendar_id(),
                    event.id()
                ))
                .query(&[("Page", page)])
                .send()
                .await?;
            let more = response.more && !response.attendees.is_empty();
            event.extend_attendees(response.attendees, more);
            page += 1;
        }
        Ok(())
    }

    pub async fn open(
        &self,
        calendar: &UnlockedCalendar,
        record: &CalendarEvent,
    ) -> Result<DecryptedEvent> {
        let author = self.author_keys(record.author()).await?;
        let pgp = *self.pgp();
        let calendar = calendar.clone();
        let address_ring = record
            .address_id()
            .and_then(|address| self.keys().address_keys(address))
            .cloned();
        let record = record.clone();
        offload(move || {
            decrypt_event(
                &pgp,
                &record,
                OpeningKeys {
                    calendar: calendar.keys(),
                    address: address_ring.as_ref(),
                    author: &author,
                },
                Some(calendar.settings()),
            )
        })
        .await
    }

    pub async fn update_attendee_status(
        &self,
        calendar: &CalendarId,
        event: &EventId,
        attendee: &AttendeeId,
        status: ParticipationStatus,
        updated_at: Timestamp,
    ) -> Result<()> {
        self.api()
            .put(format!(
                "{CALENDAR_V1}/{calendar}/events/{event}/attendees/{attendee}"
            ))
            .json(&StatusBody {
                status: status.api_status(),
                update_time: updated_at,
            })
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn update_personal_part(
        &self,
        calendar: &CalendarId,
        event: &EventId,
        notifications: Option<&[Notification]>,
        color: Option<&str>,
    ) -> Result<CalendarEvent> {
        Ok(self
            .api()
            .put(format!("{CALENDAR_V1}/{calendar}/events/{event}/personal"))
            .json(&PersonalBody {
                notifications,
                color,
            })
            .send_field("Event")
            .await?)
    }

    pub async fn alarms(
        &self,
        calendar: &CalendarId,
        from: Timestamp,
        to: Timestamp,
    ) -> Result<Vec<CalendarAlarm>> {
        let mut start = from.as_second();
        let mut visited = HashSet::new();
        let collected: Vec<Option<CalendarAlarm>> = self
            .api()
            .get(format!("{CALENDAR_V1}/{calendar}/alarms"))
            .query(&[("End", to.as_second())])
            .pages(
                Paging::cursor("Start")
                    .starting_from(start)
                    .page_size(PAGE_SIZE),
            )
            .field("Alarms")
            .collect(|alarms: Vec<CalendarAlarm>| {
                let last = alarms
                    .iter()
                    .map(|alarm| alarm.occurrence().as_second())
                    .max();
                let unseen: Vec<Option<CalendarAlarm>> = alarms
                    .into_iter()
                    .map(|alarm| visited.insert(alarm.id().clone()).then_some(alarm))
                    .collect();
                let advanced = unseen.iter().any(Option::is_some);
                let next = last.filter(|last| advanced && *last >= start);
                if let Some(next) = next {
                    start = next;
                }
                Page::new(unseen).next(next)
            })
            .await?;
        Ok(collected.into_iter().flatten().collect())
    }

    pub async fn latest_model_event(&self, calendar: &CalendarId) -> Result<ModelEventId> {
        Ok(self
            .api()
            .get(format!("{CALENDAR_V1}/{calendar}/modelevents/latest"))
            .send_field("CalendarModelEventID")
            .await?)
    }

    pub async fn model_events(
        &self,
        calendar: &CalendarId,
        since: &ModelEventId,
    ) -> Result<ModelEvents> {
        Ok(self
            .api()
            .get(format!("{CALENDAR_V1}/{calendar}/modelevents/{since}"))
            .send()
            .await?)
    }
}
