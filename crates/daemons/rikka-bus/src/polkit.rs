use std::collections::HashMap;

use serde::Serialize;
use zbus::zvariant::{self, OwnedValue, Type, Value};

pub const AUTHORITY: &str = "org.freedesktop.PolicyKit1";

#[derive(Debug, Serialize, Type)]
pub struct Subject {
    kind: String,
    details: HashMap<String, OwnedValue>,
}

impl Subject {
    pub fn session(id: &str) -> zvariant::Result<Subject> {
        Subject::new("unix-session", "session-id", id)
    }

    pub fn bus_name(name: &str) -> zvariant::Result<Subject> {
        Subject::new("system-bus-name", "name", name)
    }

    fn new(kind: &str, key: &str, value: &str) -> zvariant::Result<Subject> {
        Ok(Subject {
            kind: kind.to_owned(),
            details: HashMap::from([(key.to_owned(), OwnedValue::try_from(Value::from(value))?)]),
        })
    }
}

#[zbus::proxy(
    interface = "org.freedesktop.PolicyKit1.Authority",
    default_service = "org.freedesktop.PolicyKit1",
    default_path = "/org/freedesktop/PolicyKit1/Authority"
)]
pub trait Authority {
    fn register_authentication_agent(
        &self,
        subject: &Subject,
        locale: &str,
        object_path: &str,
    ) -> zbus::Result<()>;

    fn unregister_authentication_agent(
        &self,
        subject: &Subject,
        object_path: &str,
    ) -> zbus::Result<()>;

    fn check_authorization(
        &self,
        subject: &Subject,
        action_id: &str,
        details: HashMap<&str, &str>,
        flags: u32,
        cancellation_id: &str,
    ) -> zbus::Result<(bool, bool, HashMap<String, String>)>;

    fn cancel_check_authorization(&self, cancellation_id: &str) -> zbus::Result<()>;
}
