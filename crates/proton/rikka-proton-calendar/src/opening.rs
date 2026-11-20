use std::collections::HashMap;

use rikka_proton_api::base64;
use rikka_proton_contentline::ContentLine;
use rikka_proton_pgp::{KeyRing, Pgp, PublicKey, SessionKey, SignatureStatus, unarmor};

use crate::alarm::Alarm;
use crate::bootstrap::CalendarSettings;
use crate::calendar_event::{CalendarEvent, CardType, CommentKind, EventCard};
use crate::component::Component;
use crate::error::{Error, Result};
use crate::event::Event;
use crate::parts::{merge_parts, parse_vevents};
use crate::party::{Attendee, ParticipationStatus};
use crate::sealing::SessionKeys;

const ATTENDEE: &str = "ATTENDEE";
const COLOR: &str = "COLOR";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventVerification {
    Successful,
    NotVerified,
    Failed,
}

impl EventVerification {
    fn aggregate(statuses: &[EventVerification]) -> EventVerification {
        if statuses.is_empty() {
            return EventVerification::NotVerified;
        }
        if statuses.contains(&EventVerification::Failed) {
            return EventVerification::Failed;
        }
        if statuses
            .iter()
            .all(|status| *status == EventVerification::Successful)
        {
            return EventVerification::Successful;
        }
        EventVerification::NotVerified
    }
}

#[derive(Debug, Clone)]
pub struct DecryptedEvent {
    event: Event,
    verification: EventVerification,
    default_notifications: bool,
    session_keys: SessionKeys,
    comments: HashMap<String, String>,
}

impl DecryptedEvent {
    #[must_use]
    pub fn event(&self) -> &Event {
        &self.event
    }

    #[must_use]
    pub fn into_event(self) -> Event {
        self.event
    }

    #[must_use]
    pub fn verification(&self) -> EventVerification {
        self.verification
    }

    #[must_use]
    pub fn uses_default_notifications(&self) -> bool {
        self.default_notifications
    }

    #[must_use]
    pub fn session_keys(&self) -> &SessionKeys {
        &self.session_keys
    }

    #[must_use]
    pub fn comment(&self, token: &str) -> Option<&str> {
        self.comments.get(token).map(String::as_str)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OpeningKeys<'a> {
    pub calendar: &'a KeyRing,
    pub address: Option<&'a KeyRing>,
    pub author: &'a [PublicKey],
}

pub fn decrypt_event(
    pgp: &Pgp,
    record: &CalendarEvent,
    keys: OpeningKeys<'_>,
    settings: Option<&CalendarSettings>,
) -> Result<DecryptedEvent> {
    let session_keys = decrypt_session_keys(pgp, record, keys)?;
    let mut statuses = Vec::new();
    let mut read = |cards: &[EventCard], session_key: Option<&SessionKey>| {
        read_cards(pgp, record, cards, session_key, keys.author, &mut statuses)
    };
    let mut parts = read(record.shared_cards(), session_keys.shared.as_ref())?;
    parts.extend(read(
        record.calendar_cards(),
        session_keys.calendar.as_ref(),
    )?);
    let attendee_parts = read(record.attendee_cards(), session_keys.shared.as_ref())?;

    let mut component = merge_parts(&parts)?;
    merge_attendees(&mut component, record, &attendee_parts);
    let comments = decrypt_comments(pgp, record, session_keys.shared.as_ref());
    let default_notifications =
        apply_notifications(&mut component, pgp, record, keys.author, settings)?;
    if let Some(color) = record.color().filter(|color| !color.is_empty()) {
        component.set(ContentLine::new(COLOR, color));
    }

    Ok(DecryptedEvent {
        event: Event::from_component(component)?,
        verification: EventVerification::aggregate(&statuses),
        default_notifications,
        session_keys,
        comments,
    })
}

fn read_cards(
    pgp: &Pgp,
    record: &CalendarEvent,
    cards: &[EventCard],
    session_key: Option<&SessionKey>,
    author: &[PublicKey],
    statuses: &mut Vec<EventVerification>,
) -> Result<Vec<Component>> {
    let mut parts = Vec::new();
    for card in cards {
        parts.extend(read_card(pgp, record, card, session_key, author, statuses)?);
    }
    Ok(parts)
}

fn merge_attendees(
    component: &mut Component,
    record: &CalendarEvent,
    attendee_parts: &[Component],
) {
    component.remove(ATTENDEE);
    let statuses_by_token: HashMap<&str, i32> = record
        .attendees()
        .iter()
        .map(|attendee| (attendee.token(), attendee.status()))
        .collect();
    for part in attendee_parts {
        for line in part.all(ATTENDEE) {
            let mut attendee = Attendee::from_line(line.clone());
            if let Some(status) = attendee
                .token()
                .and_then(|token| statuses_by_token.get(token))
            {
                attendee.set_status(ParticipationStatus::from_api_status(*status));
            }
            component.push(attendee.into_line());
        }
    }
}

fn decrypt_comments(
    pgp: &Pgp,
    record: &CalendarEvent,
    shared_session_key: Option<&SessionKey>,
) -> HashMap<String, String> {
    let mut comments = HashMap::new();
    for attendee in record.attendees() {
        let Some(comment) = attendee.comment() else {
            continue;
        };
        let text = match comment.kind {
            CommentKind::Clear => Some(comment.message.clone()),
            CommentKind::EncryptedAndSigned => shared_session_key.and_then(|session| {
                let packet = base64::decode(&comment.message).ok()?;
                let plaintext = pgp
                    .decrypt()
                    .with_session_key(session.clone())
                    .message(packet)
                    .inspect_err(|error| {
                        tracing::warn!(%error, event = %record.id(), "failed to decrypt attendee comment");
                    })
                    .ok()?;
                String::from_utf8(plaintext.into_data()).ok()
            }),
        };
        if let Some(text) = text {
            comments.insert(attendee.token().to_owned(), text);
        }
    }
    comments
}

fn apply_notifications(
    component: &mut Component,
    pgp: &Pgp,
    record: &CalendarEvent,
    author: &[PublicKey],
    settings: Option<&CalendarSettings>,
) -> Result<bool> {
    component.remove_components("VALARM");
    let mut legacy_alarms = Vec::new();
    let mut legacy_statuses = Vec::new();
    for part in read_cards(
        pgp,
        record,
        record.personal_cards(),
        None,
        author,
        &mut legacy_statuses,
    )? {
        for alarm in part.children("VALARM") {
            legacy_alarms.push(Alarm::from_component(alarm.clone())?);
        }
    }
    let uses_defaults = record.notifications().is_none() && legacy_alarms.is_empty();
    match record.notifications() {
        Some(notifications) => {
            for notification in notifications {
                component.push_component(notification.to_alarm()?.into_component());
            }
        }
        None if !legacy_alarms.is_empty() => {
            for alarm in legacy_alarms {
                component.push_component(alarm.into_component());
            }
        }
        None => {
            let defaults = settings.map_or(&[][..], |settings| {
                settings.default_notifications(record.is_full_day())
            });
            for notification in defaults {
                component.push_component(notification.to_alarm()?.into_component());
            }
        }
    }
    Ok(uses_defaults)
}

fn decrypt_session_keys(
    pgp: &Pgp,
    record: &CalendarEvent,
    keys: OpeningKeys<'_>,
) -> Result<SessionKeys> {
    let decrypt_key_packet =
        |packet: &str, ring: &KeyRing, field: &'static str| -> Result<SessionKey> {
            let bytes = base64::decode(packet).map_err(|_| Error::NotBase64(field))?;
            Ok(pgp.decrypt().with_ring(ring).session_key(bytes)?)
        };
    let shared = match (record.address_key_packet(), keys.address) {
        (Some(packet), Some(ring)) => Some(decrypt_key_packet(packet, ring, "address key packet")?),
        _ => record
            .shared_key_packet()
            .map(|packet| decrypt_key_packet(packet, keys.calendar, "shared key packet"))
            .transpose()?,
    };
    let calendar = record
        .calendar_key_packet()
        .map(|packet| decrypt_key_packet(packet, keys.calendar, "calendar key packet"))
        .transpose()?;
    Ok(SessionKeys { shared, calendar })
}

fn read_card(
    pgp: &Pgp,
    record: &CalendarEvent,
    card: &EventCard,
    session: Option<&SessionKey>,
    author: &[PublicKey],
    statuses: &mut Vec<EventVerification>,
) -> Result<Vec<Component>> {
    let text = match card.kind() {
        CardType::Clear => {
            statuses.push(EventVerification::NotVerified);
            card.data().to_owned()
        }
        CardType::Signed => {
            let signature = card.signature().ok_or_else(|| {
                Error::InvalidEvent(
                    record.id().clone(),
                    "signed part has no signature".to_owned(),
                )
            })?;
            statuses.push(verify_card(pgp, card.data().as_bytes(), signature, author));
            card.data().to_owned()
        }
        CardType::Encrypted | CardType::EncryptedAndSigned => {
            let session = session.ok_or_else(|| Error::MissingSessionKey(record.id().clone()))?;
            let packet = base64::decode(card.data()).map_err(|_| Error::NotBase64("event part"))?;
            let plaintext = pgp
                .decrypt()
                .with_session_key(session.clone())
                .message(packet)
                .map_err(|error| Error::InvalidEvent(record.id().clone(), error.to_string()))?;
            let data = plaintext.into_data();
            match (card.kind(), card.signature()) {
                (CardType::EncryptedAndSigned, Some(signature)) => {
                    statuses.push(verify_card(pgp, &data, signature, author));
                }
                (CardType::EncryptedAndSigned, None) => {
                    return Err(Error::InvalidEvent(
                        record.id().clone(),
                        "encrypted and signed part has no signature".to_owned(),
                    ));
                }
                _ => statuses.push(EventVerification::NotVerified),
            }
            String::from_utf8(data)
                .map_err(|error| Error::InvalidEvent(record.id().clone(), error.to_string()))?
        }
        CardType::Other(kind) => {
            return Err(Error::InvalidEvent(
                record.id().clone(),
                format!("unknown part type {kind}"),
            ));
        }
    };
    parse_vevents(&text)
}

fn verify_card(pgp: &Pgp, data: &[u8], signature: &str, author: &[PublicKey]) -> EventVerification {
    if author.is_empty() {
        return EventVerification::NotVerified;
    }
    let status = pgp
        .verify()
        .with_keys(author.to_vec())
        .detached(data, signature);
    let status = match status {
        SignatureStatus::Invalid { .. } if needs_legacy_verification(data) => pgp
            .verify()
            .with_keys(author.to_vec())
            .as_utf8()
            .detached(strip_trailing_whitespace(data), signature),
        other => other,
    };
    match status {
        SignatureStatus::Valid { .. } => EventVerification::Successful,
        _ => EventVerification::Failed,
    }
}

fn needs_legacy_verification(data: &[u8]) -> bool {
    let text = String::from_utf8_lossy(data);
    text.contains(" \r\n") || text.replace("\r\n", "").contains('\n')
}

fn strip_trailing_whitespace(data: &[u8]) -> String {
    String::from_utf8_lossy(data)
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t', '\r']))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn armored_session_key(pgp: &Pgp, armored: &str, ring: &KeyRing) -> Result<SessionKey> {
    Ok(pgp
        .decrypt()
        .with_ring(ring)
        .session_key(unarmor(armored)?)?)
}
