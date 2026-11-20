use std::time::Duration;

use crate::cbor::Value;
use crate::ceremony::Watch;
use crate::ctaphid::{Channel, MSG};
use crate::error::{Error, Result};

const REGISTER: u8 = 0x01;
const AUTHENTICATE: u8 = 0x02;
const ENFORCE_PRESENCE: u8 = 0x03;
const CHECK_ONLY: u8 = 0x07;

const NO_ERROR: u16 = 0x9000;
const CONDITIONS_NOT_SATISFIED: u16 = 0x6985;
const WRONG_DATA: u16 = 0x6a80;

const RETRY: Duration = Duration::from_millis(150);
const MAX_HANDLE: usize = 255;
const PUBLIC_KEY: usize = 65;

pub(crate) struct RegistrationResponse {
    pub(crate) public_key: [u8; PUBLIC_KEY],
    pub(crate) handle: Vec<u8>,
    pub(crate) certificate: Vec<u8>,
    pub(crate) signature: Vec<u8>,
}

impl RegistrationResponse {
    pub(crate) fn cose_key(&self) -> Value {
        Value::map([
            (Value::from(1), Value::from(2)),
            (Value::from(3), Value::from(-7)),
            (Value::from(-1), Value::from(1)),
            (Value::from(-2), Value::bytes(&self.public_key[1..33])),
            (Value::from(-3), Value::bytes(&self.public_key[33..65])),
        ])
    }
}

pub(crate) struct AuthenticationResponse {
    pub(crate) flags: u8,
    pub(crate) counter: [u8; 4],
    pub(crate) signature: Vec<u8>,
}

pub(crate) fn is_valid_key_handle(handle: &[u8]) -> bool {
    !handle.is_empty() && handle.len() <= MAX_HANDLE
}

pub(crate) fn has_key_handle(
    channel: &mut Channel,
    application: &[u8; 32],
    handle: &[u8],
    watch: &Watch,
) -> Result<bool> {
    let data = authentication(&[0u8; 32], application, handle);
    match exchange(channel, AUTHENTICATE, CHECK_ONLY, &data, watch)? {
        (CONDITIONS_NOT_SATISFIED, _) | (NO_ERROR, _) => Ok(true),
        (WRONG_DATA, _) => Ok(false),
        (status, _) => Err(Error::U2f(status)),
    }
}

pub(crate) fn authenticate(
    channel: &mut Channel,
    challenge: &[u8; 32],
    application: &[u8; 32],
    handle: &[u8],
    watch: &Watch,
) -> Result<AuthenticationResponse> {
    let data = authentication(challenge, application, handle);
    let response = exchange_until_present(channel, AUTHENTICATE, ENFORCE_PRESENCE, &data, watch)?;
    if response.len() < 6 {
        return Err(Error::Protocol("U2F signature response is too short"));
    }
    Ok(AuthenticationResponse {
        flags: response[0],
        counter: [response[1], response[2], response[3], response[4]],
        signature: response[5..].to_vec(),
    })
}

pub(crate) fn register(
    channel: &mut Channel,
    challenge: &[u8; 32],
    application: &[u8; 32],
    watch: &Watch,
) -> Result<RegistrationResponse> {
    let data = [challenge.as_slice(), application].concat();
    let response = exchange_until_present(channel, REGISTER, 0, &data, watch)?;
    parse_registration(&response)
}

fn exchange_until_present(
    channel: &mut Channel,
    instruction: u8,
    parameter: u8,
    data: &[u8],
    watch: &Watch,
) -> Result<Vec<u8>> {
    loop {
        match exchange(channel, instruction, parameter, data, watch)? {
            (NO_ERROR, body) => return Ok(body),
            (CONDITIONS_NOT_SATISFIED, _) => {
                watch.touch();
                watch.pause(RETRY)?;
            }
            (WRONG_DATA, _) => return Err(Error::NoCredentials),
            (status, _) => return Err(Error::U2f(status)),
        }
    }
}

fn authentication(challenge: &[u8; 32], application: &[u8; 32], handle: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(65 + handle.len());
    data.extend_from_slice(challenge);
    data.extend_from_slice(application);
    data.push(handle.len() as u8);
    data.extend_from_slice(handle);
    data
}

fn exchange(
    channel: &mut Channel,
    instruction: u8,
    parameter: u8,
    data: &[u8],
    watch: &Watch,
) -> Result<(u16, Vec<u8>)> {
    let mut apdu = vec![0x00, instruction, parameter, 0x00, 0x00];
    if !data.is_empty() {
        apdu.extend_from_slice(&(data.len() as u16).to_be_bytes());
        apdu.extend_from_slice(data);
    }
    apdu.extend_from_slice(&[0x00, 0x00]);

    let mut response = channel.transact(MSG, &apdu, watch)?;
    if response.len() < 2 {
        return Err(Error::Protocol("U2F response missing status word"));
    }
    let status = response.split_off(response.len() - 2);
    Ok((u16::from_be_bytes([status[0], status[1]]), response))
}

fn parse_registration(response: &[u8]) -> Result<RegistrationResponse> {
    let truncated = || Error::Protocol("U2F registration response is truncated");

    if response.first() != Some(&0x05) {
        return Err(Error::Protocol(
            "U2F registration response missing reserved byte",
        ));
    }
    let public_key: [u8; PUBLIC_KEY] = response
        .get(1..1 + PUBLIC_KEY)
        .and_then(|key| key.try_into().ok())
        .ok_or_else(truncated)?;
    if public_key[0] != 0x04 {
        return Err(Error::Protocol(
            "U2F public key is not an uncompressed point",
        ));
    }

    let handle_at = 1 + PUBLIC_KEY;
    let handle_length = usize::from(*response.get(handle_at).ok_or_else(truncated)?);
    let handle = response
        .get(handle_at + 1..handle_at + 1 + handle_length)
        .ok_or_else(truncated)?
        .to_vec();

    let certificate_at = handle_at + 1 + handle_length;
    let certificate_length = der_length(&response[certificate_at..]).ok_or_else(truncated)?;
    let certificate = response
        .get(certificate_at..certificate_at + certificate_length)
        .ok_or_else(truncated)?
        .to_vec();
    let signature = response[certificate_at + certificate_length..].to_vec();
    if signature.is_empty() {
        return Err(truncated());
    }

    Ok(RegistrationResponse {
        public_key,
        handle,
        certificate,
        signature,
    })
}

fn der_length(bytes: &[u8]) -> Option<usize> {
    if *bytes.first()? != 0x30 {
        return None;
    }
    let first = *bytes.get(1)?;
    if first < 0x80 {
        return Some(2 + usize::from(first));
    }
    let count = usize::from(first & 0x7f);
    if count == 0 || count > 3 {
        return None;
    }
    let length = bytes
        .get(2..2 + count)?
        .iter()
        .fold(0usize, |sum, byte| (sum << 8) | usize::from(*byte));
    Some(2 + count + length)
}
