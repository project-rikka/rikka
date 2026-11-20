use rikka_secrets_protocol::names::{IS_LOCKED, NO_SESSION, NO_SUCH_OBJECT};
use zbus::message::{Header, Message};
use zbus::names::ErrorName;

const INVALID_ARGS: &str = "org.freedesktop.DBus.Error.InvalidArgs";
const NOT_SUPPORTED: &str = "org.freedesktop.DBus.Error.NotSupported";
const ACCESS_DENIED: &str = "org.freedesktop.DBus.Error.AccessDenied";
const FAILED: &str = "org.freedesktop.DBus.Error.Failed";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    IsLocked,
    NoSession,
    NoSuchObject,
    InvalidArgs,
    NotSupported,
    AccessDenied,
    Failed,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::IsLocked => IS_LOCKED,
            Kind::NoSession => NO_SESSION,
            Kind::NoSuchObject => NO_SUCH_OBJECT,
            Kind::InvalidArgs => INVALID_ARGS,
            Kind::NotSupported => NOT_SUPPORTED,
            Kind::AccessDenied => ACCESS_DENIED,
            Kind::Failed => FAILED,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Fault {
    kind: Kind,
    message: String,
}

pub(crate) type Answer<T> = std::result::Result<T, Fault>;

impl Fault {
    fn new(kind: Kind, message: impl Into<String>) -> Fault {
        Fault {
            kind,
            message: message.into(),
        }
    }

    pub(crate) fn kind(&self) -> Kind {
        self.kind
    }

    pub(crate) fn invalid(reason: impl Into<String>) -> Fault {
        Fault::new(Kind::InvalidArgs, reason)
    }

    pub(crate) fn unsupported(reason: impl Into<String>) -> Fault {
        Fault::new(Kind::NotSupported, reason)
    }

    pub(crate) fn denied(reason: impl Into<String>) -> Fault {
        Fault::new(Kind::AccessDenied, reason)
    }

    pub(crate) fn failed(reason: impl Into<String>) -> Fault {
        Fault::new(Kind::Failed, reason)
    }

    pub(crate) fn locked() -> Fault {
        Fault::new(Kind::IsLocked, "the collection is locked")
    }

    pub(crate) fn no_session(path: &str) -> Fault {
        Fault::new(Kind::NoSession, format!("no session {path}"))
    }

    pub(crate) fn no_object(what: impl std::fmt::Display) -> Fault {
        Fault::new(Kind::NoSuchObject, format!("no such object {what}"))
    }

    pub(crate) fn into_fdo(self) -> zbus::fdo::Error {
        match self.kind {
            Kind::InvalidArgs => zbus::fdo::Error::InvalidArgs(self.message),
            Kind::NotSupported => zbus::fdo::Error::NotSupported(self.message),
            Kind::AccessDenied | Kind::IsLocked => zbus::fdo::Error::AccessDenied(self.message),
            Kind::NoSuchObject => zbus::fdo::Error::UnknownObject(self.message),
            Kind::NoSession | Kind::Failed => zbus::fdo::Error::Failed(self.message),
        }
    }
}

impl zbus::DBusError for Fault {
    fn create_reply(&self, call: &Header<'_>) -> zbus::Result<Message> {
        Message::error(call, self.name())?.build(&(self.message.as_str(),))
    }

    fn name(&self) -> ErrorName<'_> {
        ErrorName::from_static_str_unchecked(self.kind.name())
    }

    fn description(&self) -> Option<&str> {
        Some(&self.message)
    }
}

impl std::fmt::Display for Fault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.kind.name(), self.message)
    }
}

impl std::error::Error for Fault {}

impl From<zbus::Error> for Fault {
    fn from(error: zbus::Error) -> Fault {
        Fault::failed(error.to_string())
    }
}

impl From<rikka_keyring_vault::Error> for Fault {
    fn from(error: rikka_keyring_vault::Error) -> Fault {
        match error {
            rikka_keyring_vault::Error::Locked => Fault::locked(),
            rikka_keyring_vault::Error::NoSuchItem(id) => Fault::no_object(format!("item {id}")),
            error => {
                tracing::error!(%error, "keyring storage failed");
                Fault::failed(error.to_string())
            }
        }
    }
}

impl From<rikka_secrets_protocol::Error> for Fault {
    fn from(error: rikka_secrets_protocol::Error) -> Fault {
        Fault::invalid(error.to_string())
    }
}
