use std::collections::BTreeMap;

use super::command::{self, GET_INFO};
use super::pin_protocol::PinProtocol;
use crate::cbor::Value;
use crate::ceremony::Watch;
use crate::ctaphid::Channel;
use crate::error::{Error, Result};

pub(crate) const FIDO_2_1: &str = "FIDO_2_1";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Info {
    versions: Vec<String>,
    extensions: Vec<String>,
    aaguid: [u8; 16],
    options: BTreeMap<String, bool>,
    max_message_size: Option<u64>,
    max_large_blob_array: Option<u64>,
    pin_protocols: Vec<u64>,
    max_credential_count: Option<u64>,
    max_credential_id_length: Option<u64>,
    transports: Vec<String>,
    algorithms: Vec<i64>,
    min_pin_length: Option<u64>,
    firmware_version: Option<u64>,
    force_pin_change: bool,
}

impl Info {
    pub(crate) fn read(channel: &mut Channel, watch: &Watch) -> Result<Info> {
        let reply = command::call(channel, GET_INFO, None, watch)?;
        Info::parse(&reply)
    }

    fn parse(reply: &Value) -> Result<Info> {
        let texts = |key: u64| -> Vec<String> {
            reply
                .get(key)
                .and_then(Value::as_array)
                .unwrap_or_default()
                .iter()
                .filter_map(Value::as_text)
                .map(str::to_owned)
                .collect()
        };
        let unsigned = |key: u64| reply.get(key).and_then(Value::as_unsigned);

        let versions = texts(0x01);
        if versions.is_empty() {
            return Err(Error::Protocol("authenticatorGetInfo missing versions"));
        }

        let aaguid = reply
            .get(0x03)
            .and_then(Value::as_bytes)
            .and_then(|bytes| <[u8; 16]>::try_from(bytes).ok())
            .ok_or(Error::Protocol("authenticatorGetInfo missing aaguid"))?;

        let options = reply
            .get(0x04)
            .and_then(Value::as_map)
            .unwrap_or_default()
            .iter()
            .filter_map(|(key, value)| Some((key.as_text()?.to_owned(), value.as_bool()?)))
            .collect();

        let pin_protocols = reply
            .get(0x06)
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_unsigned)
            .collect();

        let algorithms = reply
            .get(0x0a)
            .and_then(Value::as_array)
            .unwrap_or_default()
            .iter()
            .filter(|entry| entry.get("type").and_then(Value::as_text) == Some("public-key"))
            .filter_map(|entry| entry.get("alg").and_then(Value::as_i64))
            .collect();

        Ok(Info {
            versions,
            extensions: texts(0x02),
            aaguid,
            options,
            max_message_size: unsigned(0x05),
            max_large_blob_array: unsigned(0x0b),
            pin_protocols,
            max_credential_count: unsigned(0x07),
            max_credential_id_length: unsigned(0x08),
            transports: texts(0x09),
            algorithms,
            min_pin_length: unsigned(0x0d),
            firmware_version: unsigned(0x0e),
            force_pin_change: reply.get(0x0c).and_then(Value::as_bool) == Some(true),
        })
    }

    #[must_use]
    pub fn versions(&self) -> &[String] {
        &self.versions
    }

    #[must_use]
    pub fn supports(&self, version: &str) -> bool {
        self.versions.iter().any(|known| known == version)
    }

    #[must_use]
    pub fn extensions(&self) -> &[String] {
        &self.extensions
    }

    #[must_use]
    pub fn aaguid(&self) -> [u8; 16] {
        self.aaguid
    }

    #[must_use]
    pub fn option(&self, name: &str) -> Option<bool> {
        self.options.get(name).copied()
    }

    #[must_use]
    pub fn options(&self) -> &BTreeMap<String, bool> {
        &self.options
    }

    #[must_use]
    pub fn has_pin(&self) -> Option<bool> {
        self.option("clientPin")
    }

    #[must_use]
    pub fn has_built_in_uv(&self) -> Option<bool> {
        self.option("uv")
    }

    #[must_use]
    pub fn max_large_blob_array(&self) -> Option<u64> {
        self.max_large_blob_array
    }

    #[must_use]
    pub fn max_message_size(&self) -> Option<u64> {
        self.max_message_size
    }

    #[must_use]
    pub fn pin_protocols(&self) -> &[u64] {
        &self.pin_protocols
    }

    #[must_use]
    pub fn max_credential_count(&self) -> Option<u64> {
        self.max_credential_count
    }

    #[must_use]
    pub fn max_credential_id_length(&self) -> Option<u64> {
        self.max_credential_id_length
    }

    #[must_use]
    pub fn transports(&self) -> &[String] {
        &self.transports
    }

    #[must_use]
    pub fn algorithms(&self) -> &[i64] {
        &self.algorithms
    }

    #[must_use]
    pub fn min_pin_length(&self) -> usize {
        self.min_pin_length
            .and_then(|length| usize::try_from(length).ok())
            .unwrap_or(4)
    }

    #[must_use]
    pub fn firmware_version(&self) -> Option<u64> {
        self.firmware_version
    }

    #[must_use]
    pub fn supports_extension(&self, name: &str) -> bool {
        self.extensions.iter().any(|known| known == name)
    }

    #[must_use]
    pub fn force_pin_change(&self) -> bool {
        self.force_pin_change
    }

    pub(crate) fn pin_protocol(&self) -> Option<PinProtocol> {
        if self.pin_protocols.is_empty() {
            return self.has_pin().map(|_| PinProtocol::One);
        }
        self.pin_protocols
            .iter()
            .find_map(|number| PinProtocol::from_number(*number))
    }

    pub(crate) fn supports_pin_uv_auth_token(&self) -> bool {
        self.option("pinUvAuthToken") == Some(true)
    }

    pub(crate) fn accepts_credential_id(&self, id: &[u8]) -> bool {
        self.max_credential_id_length
            .is_none_or(|limit| id.len() as u64 <= limit)
    }

    pub(crate) fn batch(&self) -> usize {
        self.max_credential_count
            .and_then(|count| usize::try_from(count).ok())
            .filter(|count| *count > 0)
            .unwrap_or(usize::MAX)
    }
}
