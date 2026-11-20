use jiff::Timestamp;
use jiff::tz::TimeZone;
use rikka_proton_api::{Outcome, parallel_map_each};
use rikka_proton_pgp::{Pgp, PublicKey};
use serde::{Deserialize, Serialize};

use crate::calendar::Permissions;
use crate::calendar_event::CalendarEvent;
use crate::client::{CALENDAR_V1, CalendarClient};
use crate::error::{Error, Result};
use crate::event::Event;
use crate::identifier::{EventId, MemberId};
use crate::notification::Notification;
use crate::opening::DecryptedEvent;
use crate::sealing::{EventPayload, SealOptions, SealingKeys, SessionKeys, seal_event};
use crate::token::attendee_token;
use crate::unlocked::UnlockedCalendar;
use crate::zones::Zones;

const SYNC_BATCH_SIZE: usize = 10;
const DEFAULT_ATTENDEE_PERMISSIONS: u32 = 1;

#[derive(Debug, Clone, Default)]
pub struct EventOptions {
    pub default_notifications: Option<bool>,
    pub color: Option<String>,
    pub proton_attendees: Vec<(String, PublicKey)>,
    pub removed_attendees: Vec<String>,
    pub overwrite: bool,
    pub attendee_moving: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeletionReason {
    Normal,
    ChangeCalendar,
}

#[derive(Debug, Clone)]
enum Operation {
    Create {
        event: Event,
        options: EventOptions,
        session_keys: SessionKeys,
    },
    Update {
        id: EventId,
        event: Event,
        options: EventOptions,
        session_keys: SessionKeys,
    },
    Delete {
        id: EventId,
        reason: DeletionReason,
    },
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SyncedEvent {
    #[serde(rename = "Event", default)]
    event: Option<CalendarEvent>,
}

impl SyncedEvent {
    #[must_use]
    pub fn event(&self) -> Option<&CalendarEvent> {
        self.event.as_ref()
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum SyncItem {
    Create {
        #[serde(rename = "Overwrite")]
        overwrite: u8,
        #[serde(rename = "Event")]
        event: EventPayload,
    },
    Update {
        #[serde(rename = "ID")]
        id: EventId,
        #[serde(rename = "Event")]
        event: EventPayload,
    },
    Delete {
        #[serde(rename = "ID")]
        id: EventId,
        #[serde(rename = "DeletionReason")]
        reason: u8,
    },
}

#[derive(Serialize)]
struct SyncBody<'a> {
    #[serde(rename = "MemberID")]
    member: &'a MemberId,
    #[serde(rename = "IsImport", skip_serializing_if = "Option::is_none")]
    import: Option<u8>,
    #[serde(rename = "Events")]
    events: Vec<SyncItem>,
}

#[must_use]
#[derive(Debug, Clone)]
pub struct SyncBuilder {
    client: CalendarClient,
    calendar: UnlockedCalendar,
    operations: Vec<Operation>,
    import: bool,
}

impl SyncBuilder {
    pub(crate) fn new(client: CalendarClient, calendar: UnlockedCalendar) -> SyncBuilder {
        SyncBuilder {
            client,
            calendar,
            operations: Vec::new(),
            import: false,
        }
    }

    pub fn create(self, event: Event) -> SyncBuilder {
        self.create_with(event, EventOptions::default())
    }

    pub fn create_with(mut self, event: Event, options: EventOptions) -> SyncBuilder {
        self.operations.push(Operation::Create {
            event,
            options,
            session_keys: SessionKeys::default(),
        });
        self
    }

    pub fn move_here(
        mut self,
        event: Event,
        current: &DecryptedEvent,
        options: EventOptions,
    ) -> SyncBuilder {
        self.operations.push(Operation::Create {
            event,
            options,
            session_keys: current.session_keys().clone(),
        });
        self
    }

    pub fn update(self, id: &EventId, current: &DecryptedEvent, event: Event) -> SyncBuilder {
        self.update_with(id, current, event, EventOptions::default())
    }

    pub fn update_with(
        mut self,
        id: &EventId,
        current: &DecryptedEvent,
        event: Event,
        options: EventOptions,
    ) -> SyncBuilder {
        self.operations.push(Operation::Update {
            id: id.clone(),
            event,
            options,
            session_keys: current.session_keys().clone(),
        });
        self
    }

    pub fn delete(mut self, id: &EventId) -> SyncBuilder {
        self.operations.push(Operation::Delete {
            id: id.clone(),
            reason: DeletionReason::Normal,
        });
        self
    }

    pub fn delete_for_move(mut self, id: &EventId) -> SyncBuilder {
        self.operations.push(Operation::Delete {
            id: id.clone(),
            reason: DeletionReason::ChangeCalendar,
        });
        self
    }

    pub fn as_import(mut self) -> SyncBuilder {
        self.import = true;
        self
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub async fn execute(self) -> Result<Vec<Outcome<String, SyncedEvent>>> {
        let SyncBuilder {
            client,
            calendar,
            operations,
            import,
        } = self;

        let mut prepared = Vec::with_capacity(operations.len());
        for operation in operations {
            prepared.push(match operation {
                Operation::Create {
                    event,
                    options,
                    session_keys,
                } => Operation::Create {
                    event: assign_attendee_tokens(&client, event).await?,
                    options,
                    session_keys,
                },
                Operation::Update {
                    id,
                    event,
                    options,
                    session_keys,
                } => Operation::Update {
                    id,
                    event: assign_attendee_tokens(&client, event).await?,
                    options,
                    session_keys,
                },
                deletion => deletion,
            });
        }

        let keys = SealingKeys {
            address_key: calendar.address_key().clone(),
            calendar_key: calendar.primary_public_key()?,
        };
        let pgp = *client.pgp();
        let prod_id = client.prod_id().to_owned();
        let own_emails: Vec<String> = client
            .keys()
            .addresses()
            .iter()
            .map(|address| address.email().to_lowercase())
            .collect();
        let items: Vec<Result<SyncItem>> = parallel_map_each(prepared, move |operation| {
            seal_operation(&pgp, &keys, &prod_id, &own_emails, operation)
        })
        .await;
        let items = items.into_iter().collect::<Result<Vec<_>>>()?;

        let mut outcomes = Vec::with_capacity(items.len());
        let mut items = items.into_iter().peekable();
        let mut offset = 0;
        while items.peek().is_some() {
            let batch: Vec<SyncItem> = items.by_ref().take(SYNC_BATCH_SIZE).collect();
            let count = batch.len();
            let sync_response: Vec<Outcome<String, SyncedEvent>> = client
                .api()
                .put(format!(
                    "{CALENDAR_V1}/{}/events/sync",
                    calendar.calendar_id()
                ))
                .json(&SyncBody {
                    member: calendar.member().id(),
                    import: import.then_some(1),
                    events: batch,
                })
                .send_field_or_default("Responses")
                .await?;
            outcomes.extend(
                sync_response
                    .into_iter()
                    .map(|outcome| outcome.offset_by(offset)),
            );
            offset += count;
        }
        Ok(outcomes)
    }
}

async fn assign_attendee_tokens(client: &CalendarClient, event: Event) -> Result<Event> {
    let uid = event.uid().ok_or(Error::MissingUid)?;
    let attendees = event.attendees();
    let missing: Vec<String> = attendees
        .iter()
        .filter(|attendee| attendee.token().is_none_or(str::is_empty))
        .map(|attendee| attendee.email())
        .collect();
    if missing.is_empty() {
        return Ok(event);
    }

    let canonical = client.canonical_emails(&missing).await?;
    let mut updated = event;
    updated.set_attendees(
        attendees
            .into_iter()
            .map(
                |attendee| match attendee.token().is_some_and(|token| !token.is_empty()) {
                    true => attendee,
                    false => {
                        let email = attendee.email();
                        let canonical = canonical
                            .get(&email)
                            .cloned()
                            .unwrap_or_else(|| email.to_lowercase());
                        attendee.with_token(attendee_token(&canonical, &uid))
                    }
                },
            )
            .collect(),
    );
    Ok(updated)
}

pub(crate) fn clock_timestamp(pgp: &Pgp) -> Result<Timestamp> {
    let seconds = i64::try_from(pgp.now()).unwrap_or(i64::MAX);
    Ok(Timestamp::from_second(seconds)?)
}

fn seal_operation(
    pgp: &Pgp,
    keys: &SealingKeys,
    prod_id: &str,
    own_emails: &[String],
    operation: Operation,
) -> Result<SyncItem> {
    let seal = |event: Event,
                options: &EventOptions,
                session_keys: &SessionKeys,
                creating: bool|
     -> Result<EventPayload> {
        let mut event = event;
        event.ensure_stamp(clock_timestamp(pgp)?);
        let seal_options = SealOptions {
            creating,
            attendee_moving: options.attendee_moving,
            notifications: event_notifications(&event, options)?,
            color: options.color.clone().or_else(|| event.color()),
            added_proton_attendees: options.proton_attendees.clone(),
            removed_attendees: options.removed_attendees.clone(),
            prod_id: prod_id.to_owned(),
        };
        let (payload, _) = seal_event(pgp, &event, keys, session_keys, &seal_options)?;
        let payload =
            payload.with_permissions(Permissions::from_bits(DEFAULT_ATTENDEE_PERMISSIONS).bits());
        Ok(match event.organizer() {
            Some(organizer) => {
                payload.with_organizer_flag(own_emails.contains(&organizer.email().to_lowercase()))
            }
            None => payload,
        })
    };

    match operation {
        Operation::Delete { id, reason } => Ok(SyncItem::Delete {
            id,
            reason: match reason {
                DeletionReason::Normal => 0,
                DeletionReason::ChangeCalendar => 1,
            },
        }),
        Operation::Create {
            event,
            options,
            session_keys,
        } => Ok(SyncItem::Create {
            overwrite: u8::from(options.overwrite),
            event: seal(event, &options, &session_keys, true)?,
        }),
        Operation::Update {
            id,
            event,
            options,
            session_keys,
        } => {
            if session_keys.shared.is_none() {
                return Err(Error::MissingSessionKey(id));
            }
            Ok(SyncItem::Update {
                id,
                event: seal(event, &options, &session_keys, false)?.without_key_packets(),
            })
        }
    }
}

fn event_notifications(event: &Event, options: &EventOptions) -> Result<Option<Vec<Notification>>> {
    let alarms = event.alarms()?;
    let defaults = options.default_notifications.unwrap_or(alarms.is_empty());
    if defaults {
        return Ok(None);
    }
    let start = event.start()?;
    let zones = Zones::new();
    alarms
        .iter()
        .map(|alarm| Notification::from_alarm(alarm, &start, &zones, &TimeZone::UTC))
        .collect::<Result<Vec<_>>>()
        .map(Some)
}

impl CalendarClient {
    pub fn sync(&self, calendar: &UnlockedCalendar) -> SyncBuilder {
        SyncBuilder::new(self.clone(), calendar.clone())
    }
}
