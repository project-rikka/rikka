use rikka_bus::{Rule, Signals};
use rikka_nix_protocol::{
    BUSY, DENIED, INTERFACE, INVALID, OBJECT, Run, RunId, RunRecord, SERVICE, STALE, Status,
    StatusRecord,
};
use zbus::Error as Bus;
use zbus::blocking::fdo::DBusProxy;
use zbus::blocking::{Connection, Proxy};
use zbus::message::{Message, Type};
use zbus::names::BusName;

use crate::error::{Error, Result};
use crate::event::{Event, Lines};
use crate::outcome::Refusal;

const BUS: &str = "org.freedesktop.DBus";

const BUS_PATH: &str = "/org/freedesktop/DBus";

const UNKNOWN: &str = "org.freedesktop.DBus.Error.ServiceUnknown";

const NAME_OWNER_CHANGED: &str = "NameOwnerChanged";

pub(crate) const STARTED: &str = "Started";

pub(crate) const LINES: &str = "Lines";

pub(crate) const STATUS: &str = "Status";

pub(crate) const FINISHED: &str = "Finished";

pub(crate) fn connect() -> Result<Connection> {
    Connection::system()
        .map_err(|error| Error::Unavailable(format!("the system bus is not there: {error}")))
}

pub(crate) fn signals(members: &[&str]) -> Signals {
    members
        .iter()
        .fold(Signals::new(), |signals, member| {
            signals.rule(
                Rule::new()
                    .sender(SERVICE)
                    .interface(INTERFACE)
                    .path(OBJECT)
                    .member(*member),
            )
        })
        .rule(
            Rule::new()
                .sender(BUS)
                .path(BUS_PATH)
                .interface(BUS)
                .member(NAME_OWNER_CHANGED)
                .arg(0, SERVICE),
        )
}

pub(crate) fn unsubscribed(error: &Bus) -> Error {
    Error::Unavailable(format!("failed to subscribe to {SERVICE} signals: {error}"))
}

pub(crate) fn proxy(connection: &Connection) -> Result<Proxy<'static>> {
    Proxy::new(connection, SERVICE, OBJECT, INTERFACE).map_err(|error| unavailable(&error))
}

pub(crate) fn owner(connection: &Connection) -> Result<Option<String>> {
    let bus = DBusProxy::new(connection).map_err(|error| unavailable(&error))?;
    let name = BusName::try_from(SERVICE).map_err(|error| unavailable(&error.into()))?;
    match bus.get_name_owner(name) {
        Ok(owner) => Ok(Some(owner.to_string())),
        Err(zbus::fdo::Error::NameHasNoOwner(_)) => Ok(None),
        Err(error) => Err(unavailable(&error.into())),
    }
}

pub(crate) struct Follower {
    owner: Option<String>,
}

impl Follower {
    pub(crate) fn new(owner: Option<String>) -> Self {
        Self { owner }
    }

    pub(crate) fn owned_by(&mut self, owner: Option<String>) {
        self.owner = owner;
    }

    pub(crate) fn read(&mut self, message: &Message) -> Option<Event> {
        let header = message.header();
        if header.message_type() != Type::Signal {
            return None;
        }
        let sender = header.sender()?.as_str();
        let member = header.member()?.as_str();
        let body = message.body();

        if sender == BUS {
            if member != NAME_OWNER_CHANGED {
                return None;
            }
            let (name, old_owner, new_owner) = body.deserialize::<(&str, &str, &str)>().ok()?;
            if name != SERVICE {
                return None;
            }
            let left = self.owner.as_deref() == Some(old_owner) && new_owner.is_empty();
            self.owner = (!new_owner.is_empty()).then(|| new_owner.to_owned());
            return left.then_some(Event::Exited);
        }

        if self.owner.as_deref() != Some(sender)
            || header.interface()?.as_str() != INTERFACE
            || header.path()?.as_str() != OBJECT
        {
            return None;
        }
        match member {
            STARTED => Run::try_from(body.deserialize::<RunRecord>().ok()?)
                .ok()
                .map(Event::Started),
            FINISHED => Run::try_from(body.deserialize::<RunRecord>().ok()?)
                .ok()
                .map(Event::Finished),
            LINES => {
                let (run, from, lines) = body.deserialize::<(u64, u64, Vec<String>)>().ok()?;
                Some(Event::Lines(Lines {
                    run: RunId::new(run),
                    from,
                    lines,
                }))
            }
            STATUS => {
                let (run, status) = body.deserialize::<(u64, StatusRecord)>().ok()?;
                Some(Event::Status {
                    run: RunId::new(run),
                    status: Status::try_from(status).ok()?,
                })
            }
            _ => None,
        }
    }
}

pub(crate) fn refusal(error: &Bus) -> Refusal {
    match error {
        Bus::MethodError(name, detail, _) => {
            let detail = detail.clone();
            match name.as_str() {
                DENIED => Refusal::Denied,
                BUSY => Refusal::Busy,
                STALE => Refusal::Stale,
                INVALID => Refusal::Invalid(detail.unwrap_or_else(|| INVALID.to_owned())),
                UNKNOWN => Refusal::Unavailable(format!(
                    "{SERVICE} is not on the system bus; enable `rikka.enable` in the system \
                     configuration"
                )),
                other => Refusal::Unavailable(detail.unwrap_or_else(|| other.to_owned())),
            }
        }
        other => Refusal::Unavailable(other.to_string()),
    }
}

pub(crate) fn unavailable(error: &Bus) -> Error {
    match refusal(error) {
        Refusal::Denied => Error::Denied,
        refused => Error::Unavailable(refused.to_string()),
    }
}
