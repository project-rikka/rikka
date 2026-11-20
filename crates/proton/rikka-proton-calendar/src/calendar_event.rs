use jiff::Timestamp;
use rikka_proton_account::AddressId;
use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

use crate::calendar::Permissions;
use crate::identifier::{AttendeeId, CalendarId, EventId, SharedEventId};
use crate::notification::Notification;

wire_enum! {
    #[non_exhaustive]
    pub enum CardType {
        Clear = 0,
        Encrypted = 1,
        Signed = 2,
        EncryptedAndSigned = 3,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventCard {
    #[serde(rename = "Type")]
    kind: CardType,
    #[serde(rename = "Data")]
    data: String,
    #[serde(rename = "Signature", default)]
    signature: Option<String>,
    #[serde(rename = "Author", default, skip_serializing_if = "String::is_empty")]
    author: String,
}

impl EventCard {
    #[must_use]
    pub fn new(kind: CardType, data: impl Into<String>, signature: Option<String>) -> EventCard {
        EventCard {
            kind,
            data: data.into(),
            signature,
            author: String::new(),
        }
    }

    #[must_use]
    pub fn kind(&self) -> CardType {
        self.kind
    }

    #[must_use]
    pub fn data(&self) -> &str {
        &self.data
    }

    #[must_use]
    pub fn signature(&self) -> Option<&str> {
        self.signature
            .as_deref()
            .filter(|signature| !signature.is_empty())
    }

    #[must_use]
    pub fn author(&self) -> &str {
        &self.author
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
pub enum CommentKind {
    Clear,
    EncryptedAndSigned,
}

impl From<i32> for CommentKind {
    fn from(value: i32) -> CommentKind {
        match value {
            1 => CommentKind::EncryptedAndSigned,
            _ => CommentKind::Clear,
        }
    }
}

impl From<CommentKind> for i32 {
    fn from(kind: CommentKind) -> i32 {
        match kind {
            CommentKind::Clear => 0,
            CommentKind::EncryptedAndSigned => 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttendeeComment {
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "Type")]
    pub kind: CommentKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttendeeRecord {
    #[serde(rename = "ID", default)]
    id: AttendeeId,
    #[serde(rename = "Token", default)]
    token: String,
    #[serde(rename = "Status", default)]
    status: i32,
    #[serde(
        rename = "UpdateTime",
        default,
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    update_time: Option<Timestamp>,
    #[serde(rename = "Comment", default)]
    comment: Option<AttendeeComment>,
}

impl AttendeeRecord {
    #[must_use]
    pub fn id(&self) -> &AttendeeId {
        &self.id
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }

    #[must_use]
    pub fn status(&self) -> i32 {
        self.status
    }

    #[must_use]
    pub fn update_time(&self) -> Option<Timestamp> {
        self.update_time
    }

    #[must_use]
    pub fn comment(&self) -> Option<&AttendeeComment> {
        self.comment.as_ref()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AttendeesInfo {
    #[serde(rename = "Attendees", default)]
    pub(crate) attendees: Vec<AttendeeRecord>,
    #[serde(rename = "MoreAttendees", default, with = "rikka_proton_api::flag")]
    pub(crate) more: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEvent {
    #[serde(rename = "ID", default)]
    id: EventId,
    #[serde(rename = "SharedEventID", default)]
    shared_event_id: SharedEventId,
    #[serde(rename = "CalendarID", default)]
    calendar_id: CalendarId,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(
        rename = "ModifyTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    modify_time: Timestamp,
    #[serde(rename = "Permissions", default)]
    permissions: Permissions,
    #[serde(rename = "IsOrganizer", default, with = "rikka_proton_api::flag")]
    is_organizer: bool,
    #[serde(
        rename = "IsProtonProtonInvite",
        default,
        with = "rikka_proton_api::flag"
    )]
    is_proton_invite: bool,
    #[serde(
        rename = "IsPersonalSingleEdit",
        default,
        with = "rikka_proton_api::flag"
    )]
    is_personal_single_edit: bool,
    #[serde(rename = "Author", default)]
    author: String,
    #[serde(rename = "Color", default)]
    color: Option<String>,
    #[serde(rename = "StartTime", default, with = "rikka_proton_api::unix_seconds")]
    start_time: Timestamp,
    #[serde(rename = "StartTimezone", default)]
    start_timezone: String,
    #[serde(rename = "EndTime", default, with = "rikka_proton_api::unix_seconds")]
    end_time: Timestamp,
    #[serde(rename = "EndTimezone", default)]
    end_timezone: String,
    #[serde(rename = "FullDay", default, with = "rikka_proton_api::flag")]
    full_day: bool,
    #[serde(rename = "RRule", default)]
    rrule: Option<String>,
    #[serde(rename = "UID", default)]
    uid: String,
    #[serde(
        rename = "RecurrenceID",
        default,
        with = "rikka_proton_api::unix_seconds::optional"
    )]
    recurrence_id: Option<Timestamp>,
    #[serde(rename = "Exdates", default)]
    exdates: Vec<i64>,
    #[serde(rename = "CalendarKeyPacket", default)]
    calendar_key_packet: Option<String>,
    #[serde(rename = "CalendarEvents", default)]
    calendar_cards: Vec<EventCard>,
    #[serde(rename = "SharedKeyPacket", default)]
    shared_key_packet: Option<String>,
    #[serde(rename = "AddressKeyPacket", default)]
    address_key_packet: Option<String>,
    #[serde(rename = "AddressID", default)]
    address_id: Option<AddressId>,
    #[serde(rename = "SharedEvents", default)]
    shared_cards: Vec<EventCard>,
    #[serde(
        rename = "PersonalEvents",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    personal_cards: Vec<EventCard>,
    #[serde(rename = "Notifications", default)]
    notifications: Option<Vec<Notification>>,
    #[serde(rename = "AttendeesEvents", default)]
    attendee_cards: Vec<EventCard>,
    #[serde(rename = "AttendeesInfo", default)]
    attendees_info: AttendeesInfo,
}

impl CalendarEvent {
    #[must_use]
    pub fn id(&self) -> &EventId {
        &self.id
    }

    #[must_use]
    pub fn shared_event_id(&self) -> &SharedEventId {
        &self.shared_event_id
    }

    #[must_use]
    pub fn calendar_id(&self) -> &CalendarId {
        &self.calendar_id
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn modify_time(&self) -> Timestamp {
        self.modify_time
    }

    #[must_use]
    pub fn permissions(&self) -> Permissions {
        self.permissions
    }

    #[must_use]
    pub fn is_organizer(&self) -> bool {
        self.is_organizer
    }

    #[must_use]
    pub fn is_proton_invite(&self) -> bool {
        self.is_proton_invite
    }

    #[must_use]
    pub fn is_personal_single_edit(&self) -> bool {
        self.is_personal_single_edit
    }

    #[must_use]
    pub fn author(&self) -> &str {
        &self.author
    }

    #[must_use]
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    #[must_use]
    pub fn start_time(&self) -> Timestamp {
        self.start_time
    }

    #[must_use]
    pub fn start_timezone(&self) -> &str {
        &self.start_timezone
    }

    #[must_use]
    pub fn end_time(&self) -> Timestamp {
        self.end_time
    }

    #[must_use]
    pub fn end_timezone(&self) -> &str {
        &self.end_timezone
    }

    #[must_use]
    pub fn is_full_day(&self) -> bool {
        self.full_day
    }

    #[must_use]
    pub fn rrule(&self) -> Option<&str> {
        self.rrule.as_deref()
    }

    #[must_use]
    pub fn uid(&self) -> &str {
        &self.uid
    }

    #[must_use]
    pub fn recurrence_id(&self) -> Option<Timestamp> {
        self.recurrence_id
    }

    #[must_use]
    pub fn exdates(&self) -> &[i64] {
        &self.exdates
    }

    #[must_use]
    pub fn calendar_key_packet(&self) -> Option<&str> {
        self.calendar_key_packet
            .as_deref()
            .filter(|packet| !packet.is_empty())
    }

    #[must_use]
    pub fn shared_key_packet(&self) -> Option<&str> {
        self.shared_key_packet
            .as_deref()
            .filter(|packet| !packet.is_empty())
    }

    #[must_use]
    pub fn address_key_packet(&self) -> Option<&str> {
        self.address_key_packet
            .as_deref()
            .filter(|packet| !packet.is_empty())
    }

    #[must_use]
    pub fn address_id(&self) -> Option<&AddressId> {
        self.address_id.as_ref()
    }

    #[must_use]
    pub fn calendar_cards(&self) -> &[EventCard] {
        &self.calendar_cards
    }

    #[must_use]
    pub fn shared_cards(&self) -> &[EventCard] {
        &self.shared_cards
    }

    #[must_use]
    pub fn personal_cards(&self) -> &[EventCard] {
        &self.personal_cards
    }

    #[must_use]
    pub fn attendee_cards(&self) -> &[EventCard] {
        &self.attendee_cards
    }

    #[must_use]
    pub fn notifications(&self) -> Option<&[Notification]> {
        self.notifications.as_deref()
    }

    #[must_use]
    pub fn attendees(&self) -> &[AttendeeRecord] {
        &self.attendees_info.attendees
    }

    #[must_use]
    pub fn has_more_attendees(&self) -> bool {
        self.attendees_info.more
    }

    pub(crate) fn extend_attendees(&mut self, more: Vec<AttendeeRecord>, has_more: bool) {
        self.attendees_info.attendees.extend(more);
        self.attendees_info.more = has_more;
    }
}
