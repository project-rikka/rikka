use zeroize::Zeroizing;

use super::command::{self, CLIENT_PIN};
use super::pin_protocol::{PinProtocol, SharedSecret};
use crate::cbor::Value;
use crate::ceremony::{Pin, Watch};
use crate::crypto;
use crate::ctaphid::Channel;
use crate::error::{Error, Result};
use crate::status::CtapStatus;

const GET_PIN_RETRIES: u8 = 0x01;
const GET_KEY_AGREEMENT: u8 = 0x02;
const SET_PIN: u8 = 0x03;
const CHANGE_PIN: u8 = 0x04;
const GET_PIN_TOKEN: u8 = 0x05;
const GET_TOKEN_USING_UV: u8 = 0x06;
const GET_UV_RETRIES: u8 = 0x07;
const GET_TOKEN_USING_PIN: u8 = 0x09;

pub(crate) const MAKE_CREDENTIAL_PERMISSION: u8 = 0x01;
pub(crate) const GET_ASSERTION_PERMISSION: u8 = 0x02;

pub(crate) struct Token {
    protocol: PinProtocol,
    token: Zeroizing<Vec<u8>>,
}

impl Token {
    pub(crate) fn protocol(&self) -> PinProtocol {
        self.protocol
    }

    pub(crate) fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.protocol.authenticate(&self.token, message)
    }
}

pub(crate) struct Scope<'a> {
    pub(crate) permissions: u8,
    pub(crate) rp_id: &'a str,
}

pub(crate) fn pin_retries(
    channel: &mut Channel,
    protocol: PinProtocol,
    watch: &Watch,
) -> Result<u32> {
    let reply = command::call(
        channel,
        CLIENT_PIN,
        Some(&request(protocol, GET_PIN_RETRIES, [])),
        watch,
    )?;
    count(&reply, 0x03)
}

pub(crate) fn uv_retries(
    channel: &mut Channel,
    protocol: PinProtocol,
    watch: &Watch,
) -> Result<u32> {
    let reply = command::call(
        channel,
        CLIENT_PIN,
        Some(&request(protocol, GET_UV_RETRIES, [])),
        watch,
    )?;
    count(&reply, 0x05)
}

pub(crate) fn pin_token(
    channel: &mut Channel,
    protocol: PinProtocol,
    pin: &Pin,
    scope: Option<Scope<'_>>,
    watch: &Watch,
) -> Result<Token> {
    let (platform, secret) = key_agreement(channel, protocol, watch)?;
    let pin_hash_enc = encrypted_hash(&secret, pin)?;

    let mut fields = vec![(0x03, platform), (0x06, Value::Bytes(pin_hash_enc))];
    let subcommand = match scope {
        Some(scope) => {
            fields.push((0x09, Value::from(scope.permissions)));
            fields.push((0x0a, Value::from(scope.rp_id)));
            GET_TOKEN_USING_PIN
        }
        None => GET_PIN_TOKEN,
    };

    let reply = command::call(
        channel,
        CLIENT_PIN,
        Some(&request(protocol, subcommand, fields)),
        watch,
    )?;
    open_token(&secret, &reply)
}

pub(crate) fn uv_token(
    channel: &mut Channel,
    protocol: PinProtocol,
    scope: Scope<'_>,
    watch: &Watch,
) -> Result<Token> {
    let (platform, secret) = key_agreement(channel, protocol, watch)?;
    let fields = [
        (0x03, platform),
        (0x09, Value::from(scope.permissions)),
        (0x0a, Value::from(scope.rp_id)),
    ];

    let reply = command::call(
        channel,
        CLIENT_PIN,
        Some(&request(protocol, GET_TOKEN_USING_UV, fields)),
        watch,
    )?;
    open_token(&secret, &reply)
}

pub(crate) fn set_pin(
    channel: &mut Channel,
    protocol: PinProtocol,
    new: &Pin,
    watch: &Watch,
) -> Result<()> {
    let (platform, secret) = key_agreement(channel, protocol, watch)?;
    let new_pin_enc = secret.encrypt(new.padded().as_slice())?;
    let param = secret.authenticate(&new_pin_enc);
    let fields = [
        (0x03, platform),
        (0x04, Value::Bytes(param)),
        (0x05, Value::Bytes(new_pin_enc)),
    ];

    command::call(
        channel,
        CLIENT_PIN,
        Some(&request(protocol, SET_PIN, fields)),
        watch,
    )
    .map(drop)
}

pub(crate) fn change_pin(
    channel: &mut Channel,
    protocol: PinProtocol,
    current: &Pin,
    new: &Pin,
    watch: &Watch,
) -> Result<()> {
    let (platform, secret) = key_agreement(channel, protocol, watch)?;
    let new_pin_enc = secret.encrypt(new.padded().as_slice())?;
    let pin_hash_enc = encrypted_hash(&secret, current)?;
    let param = secret.authenticate(&[new_pin_enc.as_slice(), &pin_hash_enc].concat());
    let fields = [
        (0x03, platform),
        (0x04, Value::Bytes(param)),
        (0x05, Value::Bytes(new_pin_enc)),
        (0x06, Value::Bytes(pin_hash_enc)),
    ];

    command::call(
        channel,
        CLIENT_PIN,
        Some(&request(protocol, CHANGE_PIN, fields)),
        watch,
    )
    .map(drop)
}

fn encrypted_hash(secret: &SharedSecret, pin: &Pin) -> Result<Vec<u8>> {
    let pin_hash = Zeroizing::new(crypto::sha256(pin.as_bytes()));
    secret.encrypt(&pin_hash[..16])
}

pub(crate) fn key_agreement(
    channel: &mut Channel,
    protocol: PinProtocol,
    watch: &Watch,
) -> Result<(Value, SharedSecret)> {
    let reply = command::call(
        channel,
        CLIENT_PIN,
        Some(&request(protocol, GET_KEY_AGREEMENT, [])),
        watch,
    )?;
    let peer = reply
        .get(0x01)
        .ok_or(Error::Protocol("keyAgreement response missing key"))?;
    protocol.encapsulate(peer)
}

fn open_token(secret: &SharedSecret, reply: &Value) -> Result<Token> {
    let sealed = reply
        .get(0x02)
        .and_then(Value::as_bytes)
        .ok_or(Error::Protocol("PIN response missing token"))?;
    let token = secret.decrypt(sealed)?;
    if !matches!(token.len(), 16 | 32) {
        return Err(Error::Protocol("invalid PIN token length"));
    }
    Ok(Token {
        protocol: secret.protocol(),
        token,
    })
}

fn request(
    protocol: PinProtocol,
    subcommand: u8,
    fields: impl IntoIterator<Item = (i32, Value)>,
) -> Value {
    let mut entries = vec![
        (Value::from(0x01), Value::from(protocol.number())),
        (Value::from(0x02), Value::from(subcommand)),
    ];
    entries.extend(
        fields
            .into_iter()
            .map(|(key, value)| (Value::from(key), value)),
    );
    Value::Map(entries)
}

fn count(reply: &Value, key: u64) -> Result<u32> {
    reply
        .get(key)
        .and_then(Value::as_unsigned)
        .and_then(|count| u32::try_from(count).ok())
        .ok_or(Error::Protocol("retries response missing count"))
}

pub(crate) fn pin_failure(error: Error, min_length: usize) -> Error {
    match error {
        Error::Ctap(CtapStatus::PIN_POLICY_VIOLATION) => Error::PinPolicyViolation { min_length },
        Error::Ctap(CtapStatus::PIN_BLOCKED) => Error::PinBlocked,
        Error::Ctap(CtapStatus::PIN_AUTH_BLOCKED) => Error::PinAuthBlocked,
        Error::Ctap(CtapStatus::PIN_NOT_SET) => Error::PinNotSet,
        Error::Ctap(CtapStatus::NOT_ALLOWED) => Error::PinAlreadySet,
        other => other,
    }
}
