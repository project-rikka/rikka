use rikka_proton_api::base64;
use rikka_proton_pgp::{Pgp, PrivateKey, PublicKey, SessionKey};
use serde::Serialize;

use crate::calendar_event::{AttendeeComment, CardType, EventCard};
use crate::error::{Error, Result};
use crate::event::Event;
use crate::notification::Notification;
use crate::parts::{ClearAttendee, Parts, split_event};

#[derive(Debug, Clone, Default)]
pub struct SessionKeys {
    pub shared: Option<SessionKey>,
    pub calendar: Option<SessionKey>,
}

#[derive(Debug, Clone)]
pub struct SealingKeys {
    pub address_key: PrivateKey,
    pub calendar_key: PublicKey,
}

#[derive(Debug, Clone, Default)]
pub struct SealOptions {
    pub creating: bool,
    pub attendee_moving: bool,
    pub notifications: Option<Vec<Notification>>,
    pub color: Option<String>,
    pub added_proton_attendees: Vec<(String, PublicKey)>,
    pub removed_attendees: Vec<String>,
    pub prod_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClearAttendeeBody {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "Status")]
    status: i32,
    #[serde(rename = "Comment")]
    comment: Option<AttendeeComment>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AddedAttendee {
    #[serde(rename = "Email")]
    email: String,
    #[serde(rename = "AddressKeyPacket")]
    key_packet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventPayload {
    #[serde(rename = "Notifications")]
    notifications: Option<Vec<Notification>>,
    #[serde(rename = "Color")]
    color: Option<String>,
    #[serde(rename = "SharedKeyPacket", skip_serializing_if = "Option::is_none")]
    shared_key_packet: Option<String>,
    #[serde(rename = "SharedEventContent", skip_serializing_if = "Option::is_none")]
    shared_content: Option<Vec<EventCard>>,
    #[serde(rename = "CalendarKeyPacket", skip_serializing_if = "Option::is_none")]
    calendar_key_packet: Option<String>,
    #[serde(
        rename = "CalendarEventContent",
        skip_serializing_if = "Option::is_none"
    )]
    calendar_content: Option<Vec<EventCard>>,
    #[serde(
        rename = "AttendeesEventContent",
        skip_serializing_if = "Option::is_none"
    )]
    attendees_content: Option<Vec<EventCard>>,
    #[serde(rename = "Attendees", skip_serializing_if = "Option::is_none")]
    attendees: Option<Vec<ClearAttendeeBody>>,
    #[serde(
        rename = "RemovedAttendeeAddresses",
        skip_serializing_if = "Option::is_none"
    )]
    removed_attendees: Option<Vec<String>>,
    #[serde(
        rename = "AddedProtonAttendees",
        skip_serializing_if = "Option::is_none"
    )]
    added_attendees: Option<Vec<AddedAttendee>>,
    #[serde(rename = "Permissions", skip_serializing_if = "Option::is_none")]
    permissions: Option<u32>,
    #[serde(rename = "IsOrganizer", skip_serializing_if = "Option::is_none")]
    is_organizer: Option<u8>,
}

impl EventPayload {
    #[must_use]
    pub fn has_shared_content(&self) -> bool {
        self.shared_content.is_some()
    }

    #[must_use]
    pub fn has_calendar_content(&self) -> bool {
        self.calendar_content.is_some()
    }

    #[must_use]
    pub fn has_attendees_content(&self) -> bool {
        self.attendees_content.is_some()
    }

    pub(crate) fn with_permissions(mut self, permissions: u32) -> EventPayload {
        self.permissions = Some(permissions);
        self
    }

    pub(crate) fn with_organizer_flag(mut self, organizer: bool) -> EventPayload {
        self.is_organizer = Some(u8::from(organizer));
        self
    }

    pub(crate) fn without_key_packets(mut self) -> EventPayload {
        self.shared_key_packet = None;
        self.calendar_key_packet = None;
        self
    }
}

pub fn seal_event(
    pgp: &Pgp,
    event: &Event,
    keys: &SealingKeys,
    previous_keys: &SessionKeys,
    options: &SealOptions,
) -> Result<(EventPayload, SessionKeys)> {
    let Parts {
        shared_signed,
        shared_encrypted,
        calendar_signed,
        calendar_encrypted,
        attendees_encrypted,
        attendees_clear,
    } = split_event(event, &options.prod_id);
    let sealer = CardSealer { pgp, keys };

    let shared = previous_keys
        .shared
        .clone()
        .unwrap_or_else(SessionKey::generate);
    let calendar = match (
        &calendar_encrypted,
        &previous_keys.calendar,
        options.creating,
    ) {
        (None, _, _) => None,
        (Some(_), Some(session_key), _) => Some(session_key.clone()),
        (Some(_), None, true) => Some(SessionKey::generate()),
        (Some(_), None, false) => return Err(Error::CalendarPartWithoutKey),
    };

    let shared_content = match options.attendee_moving {
        true => None,
        false => Some(vec![
            sealer.signed_card(&shared_signed)?,
            sealer.encrypted_card(&shared_encrypted, &shared)?,
        ]),
    };

    let mut calendar_cards = Vec::new();
    if let Some(signed) = &calendar_signed {
        calendar_cards.push(sealer.signed_card(signed)?);
    }
    if let (Some(encrypted), Some(session)) = (&calendar_encrypted, &calendar) {
        calendar_cards.push(sealer.encrypted_card(encrypted, session)?);
    }

    let attendees_content = match (&attendees_encrypted, options.attendee_moving) {
        (Some(part), false) => Some(vec![sealer.encrypted_card(part, &shared)?]),
        _ => None,
    };
    let attendees = (!options.attendee_moving && !attendees_clear.is_empty())
        .then(|| clear_attendee_bodies(&attendees_clear));
    let added_attendees = added_attendees(pgp, &options.added_proton_attendees, &shared)?;

    let payload = EventPayload {
        notifications: options.notifications.clone(),
        color: options.color.clone(),
        shared_key_packet: options
            .creating
            .then(|| sealer.key_packet(&shared))
            .transpose()?,
        shared_content,
        calendar_key_packet: match (&calendar, options.creating) {
            (Some(session), true) => Some(sealer.key_packet(session)?),
            _ => None,
        },
        calendar_content: (!calendar_cards.is_empty()).then_some(calendar_cards),
        attendees_content,
        attendees,
        removed_attendees: (!options.removed_attendees.is_empty())
            .then(|| options.removed_attendees.clone()),
        added_attendees: (!added_attendees.is_empty()).then_some(added_attendees),
        permissions: None,
        is_organizer: None,
    };

    Ok((
        payload,
        SessionKeys {
            shared: Some(shared),
            calendar,
        },
    ))
}

struct CardSealer<'a> {
    pgp: &'a Pgp,
    keys: &'a SealingKeys,
}

impl CardSealer<'_> {
    fn key_packet(&self, session: &SessionKey) -> Result<String> {
        Ok(base64::encode(
            self.pgp
                .encrypt()
                .to(self.keys.calendar_key.clone())
                .key_packets(session)?,
        ))
    }

    fn sign(&self, data: &str) -> Result<String> {
        Ok(self
            .pgp
            .sign()
            .with_key(self.keys.address_key.clone())
            .armored_detached(data.as_bytes())?)
    }

    fn signed_card(&self, data: &str) -> Result<EventCard> {
        Ok(EventCard::new(
            CardType::Signed,
            data.to_owned(),
            Some(self.sign(data)?),
        ))
    }

    fn encrypted_card(&self, data: &str, session: &SessionKey) -> Result<EventCard> {
        let packet = self
            .pgp
            .encrypt()
            .with_session_key(session.clone())
            .message(data.as_bytes())?;
        Ok(EventCard::new(
            CardType::EncryptedAndSigned,
            base64::encode(packet),
            Some(self.sign(data)?),
        ))
    }
}

fn clear_attendee_bodies(attendees: &[ClearAttendee]) -> Vec<ClearAttendeeBody> {
    attendees
        .iter()
        .map(|clear| ClearAttendeeBody {
            token: clear.token.clone(),
            status: clear.status,
            comment: None,
        })
        .collect()
}

fn added_attendees(
    pgp: &Pgp,
    attendees: &[(String, PublicKey)],
    shared: &SessionKey,
) -> Result<Vec<AddedAttendee>> {
    attendees
        .iter()
        .map(|(email, key)| {
            Ok(AddedAttendee {
                email: email.clone(),
                key_packet: base64::encode(pgp.encrypt().to(key.clone()).key_packets(shared)?),
            })
        })
        .collect()
}
