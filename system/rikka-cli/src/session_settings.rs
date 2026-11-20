use std::collections::BTreeMap;

use rikka_seam::Request;
use rikka_settings::seam::{CAPABILITY, protocol};
use rikka_settings::{Domain, Schema, Source, Value, Values};
use serde::de::DeserializeOwned;

use crate::error::Result;
use crate::session::{CapabilitySocket, ReplyExt};

#[derive(Debug)]
pub(crate) struct SessionSettings {
    socket: CapabilitySocket,
}

impl SessionSettings {
    pub(crate) fn connect() -> Result<SessionSettings> {
        Ok(SessionSettings {
            socket: CapabilitySocket::connect(CAPABILITY)?,
        })
    }

    pub(crate) fn domains(&self) -> Result<Vec<Domain>> {
        self.ask(protocol::domains())
    }

    pub(crate) fn schema(&self, domain: &Domain) -> Result<Schema> {
        self.ask(protocol::describe(domain.as_str()))
    }

    pub(crate) fn values(&self, domain: &Domain) -> Result<Values> {
        self.ask(protocol::read(domain.as_str()))
    }

    pub(crate) fn sources(&self, domain: &Domain) -> Result<BTreeMap<String, Source>> {
        self.ask(protocol::sources(domain.as_str()))
    }

    pub(crate) fn get(&self, domain: &Domain, key: &str) -> Result<Value> {
        self.ask(protocol::get(domain.as_str(), key))
    }

    pub(crate) fn set(&self, domain: &Domain, key: &str, value: Value) -> Result<()> {
        self.ask::<Values>(protocol::set(domain.as_str(), key, value))?;
        Ok(())
    }

    pub(crate) fn reset(&self, domain: &Domain, key: &str) -> Result<()> {
        self.ask::<Values>(protocol::reset(domain.as_str(), key))?;
        Ok(())
    }

    fn ask<T: DeserializeOwned>(&self, request: Request) -> Result<T> {
        self.socket.client().call(request).decode()
    }
}
