use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::account::Account;
use crate::error::Result;
use crate::feed::EventPage;
use crate::identifier::EventId;
use crate::refresh::Refresh;

const EVENTS_PATH: &str = "core/v4/events";

#[derive(Debug, Clone, Default, Deserialize)]
struct Envelope {
    #[serde(rename = "EventID", default)]
    id: EventId,
    #[serde(rename = "Refresh", default)]
    refresh: Refresh,
    #[serde(
        rename = "More",
        default,
        deserialize_with = "rikka_proton_api::flag::deserialize"
    )]
    more: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CoreEvent {
    envelope: Envelope,
    wire: Value,
}

impl CoreEvent {
    #[must_use]
    pub fn id(&self) -> &EventId {
        &self.envelope.id
    }

    #[must_use]
    pub fn refresh(&self) -> Refresh {
        self.envelope.refresh
    }

    #[must_use]
    pub fn has_more(&self) -> bool {
        self.envelope.more
    }

    #[must_use]
    pub fn touches_account(&self) -> bool {
        ["User", "Addresses"]
            .iter()
            .any(|section| match self.wire.get(section) {
                None | Some(Value::Null) => false,
                Some(Value::Array(changes)) => !changes.is_empty(),
                Some(_) => true,
            })
    }

    pub fn project<T: DeserializeOwned>(&self) -> serde_json::Result<T> {
        T::deserialize(&self.wire)
    }
}

impl EventPage for CoreEvent {
    fn next_id(&self) -> &str {
        self.envelope.id.as_str()
    }

    fn has_more(&self) -> bool {
        self.envelope.more
    }
}

impl<'de> Deserialize<'de> for CoreEvent {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<CoreEvent, D::Error> {
        let wire = Value::deserialize(deserializer)?;
        let envelope = Envelope::deserialize(&wire).map_err(D::Error::custom)?;
        Ok(CoreEvent { envelope, wire })
    }
}

impl Account {
    pub async fn latest_event(&self) -> Result<EventId> {
        Ok(self
            .api()
            .get(format!("{EVENTS_PATH}/latest"))
            .send_field("EventID")
            .await?)
    }

    pub async fn core_events<T: DeserializeOwned>(&self, since: &EventId) -> Result<T> {
        Ok(self
            .api()
            .get(format!("{EVENTS_PATH}/{since}"))
            .send()
            .await?)
    }
}
