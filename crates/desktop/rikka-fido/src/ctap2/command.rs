use crate::cbor::Value;
use crate::ceremony::Watch;
use crate::ctaphid::{CBOR, Channel};
use crate::error::{Error, Result};
use crate::status::CtapStatus;

pub(crate) const MAKE_CREDENTIAL: u8 = 0x01;
pub(crate) const GET_ASSERTION: u8 = 0x02;
pub(crate) const GET_INFO: u8 = 0x04;
pub(crate) const CLIENT_PIN: u8 = 0x06;
pub(crate) const GET_NEXT_ASSERTION: u8 = 0x08;
pub(crate) const SELECTION: u8 = 0x0b;
pub(crate) const LARGE_BLOBS: u8 = 0x0c;

pub(crate) fn call(
    channel: &mut Channel,
    command: u8,
    parameters: Option<&Value>,
    watch: &Watch,
) -> Result<Value> {
    let mut payload = vec![command];
    if let Some(parameters) = parameters {
        payload.extend_from_slice(&parameters.encode());
    }

    let reply = channel.transact(CBOR, &payload, watch)?;
    let (&status, body) = reply
        .split_first()
        .ok_or(Error::Protocol("CTAP2 reply is empty"))?;

    match CtapStatus::new(status) {
        CtapStatus::KEEPALIVE_CANCEL => Err(Error::Cancelled),
        _ if status != 0 => Err(Error::Ctap(CtapStatus::new(status))),
        _ if body.is_empty() => Ok(Value::Map(Vec::new())),
        _ => Ok(Value::decode(body)?),
    }
}

pub(crate) fn bytes(map: &Value, key: u64, missing: &'static str) -> Result<Vec<u8>> {
    map.get(key)
        .and_then(Value::as_bytes)
        .map(<[u8]>::to_vec)
        .ok_or(Error::Protocol(missing))
}
