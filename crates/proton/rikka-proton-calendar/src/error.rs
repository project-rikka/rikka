use crate::identifier::{CalendarId, EventId};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Api(#[from] rikka_proton_api::Error),

    #[error(transparent)]
    Account(#[from] rikka_proton_account::Error),

    #[error(transparent)]
    Pgp(#[from] rikka_proton_pgp::Error),

    #[error(transparent)]
    Mail(#[from] rikka_proton_mail::Error),

    #[error(transparent)]
    Contacts(#[from] rikka_proton_contacts::Error),

    #[error("cannot encrypt to {0}: {1}")]
    RecipientRefused(String, rikka_proton_contacts::PreferenceError),

    #[error(transparent)]
    ContentLine(#[from] rikka_proton_contentline::Error),

    #[error(transparent)]
    Time(#[from] jiff::Error),

    #[error("unbalanced BEGIN/END for component {0}")]
    Unbalanced(String),

    #[error("invalid {0} value {1:?}")]
    InvalidValue(String, String),

    #[error("{0} component is missing {1}")]
    MissingProperty(&'static str, &'static str),

    #[error("expected {0}, found {1}")]
    WrongComponent(&'static str, String),

    #[error("unknown time zone {0}")]
    UnknownTimeZone(String),

    #[error("date out of range")]
    OutOfRange,

    #[error("invalid recurrence rule: {0}")]
    InvalidRule(String),

    #[error("no VCALENDAR found")]
    NoCalendar,

    #[error("cannot import calendar method {0}")]
    UnsupportedMethod(String),

    #[error("calendar has no events")]
    NoEvents,

    #[error("calendar has {0} events, import limit is {1}")]
    TooManyEvents(usize, usize),

    #[error("no member of calendar {0} for this account")]
    NotMember(CalendarId),

    #[error("calendar {0} has no passphrase for member")]
    NoMemberPassphrase(CalendarId),

    #[error("failed to unlock any key of calendar {0}")]
    NoCalendarKey(CalendarId),

    #[error("calendar {0} has no primary key")]
    NoPrimaryCalendarKey(CalendarId),

    #[error("event {0} has no shared key packet")]
    NoSharedKeyPacket(EventId),

    #[error("event {0} part is missing its session key")]
    MissingSessionKey(EventId),

    #[error("update has a calendar part but event has no calendar session key")]
    CalendarPartWithoutKey,

    #[error("invalid event {0}: {1}")]
    InvalidEvent(EventId, String),

    #[error("event has no UID")]
    MissingUid,

    #[error("{0} is not valid base64")]
    NotBase64(&'static str),

    #[error("invalid calendar passphrase signature")]
    PassphraseSignature,

    #[error("invitation to {0} has no attendee for address")]
    NotInvited(String),

    #[error("invitation has no organizer")]
    NoOrganizer,

    #[error("invited address {0} has no encryption key")]
    NoInviteeKey(String),

    #[error("unsupported iTIP method {0}")]
    UnexpectedMethod(String),

    #[error("message has no recipients")]
    NoRecipient,
}

impl Error {
    #[must_use]
    pub fn api(&self) -> Option<&rikka_proton_api::Error> {
        match self {
            Error::Api(error) => Some(error),
            Error::Account(error) => error.api(),
            Error::Mail(error) => error.api(),
            Error::Contacts(error) => error.api(),
            _ => None,
        }
    }
}
