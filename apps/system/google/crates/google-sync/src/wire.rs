use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rand::RngCore;

const WINDOWS_EPOCH_OFFSET_MICROS: i64 = 11_644_473_600_000_000;
const MICROS_PER_MILLI: i64 = 1000;
const PADDING: usize = 256;

#[must_use]
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

#[must_use]
pub const fn windows_micros_from_unix_ms(unix_ms: i64) -> i64 {
    unix_ms * MICROS_PER_MILLI + WINDOWS_EPOCH_OFFSET_MICROS
}

#[must_use]
pub const fn unix_ms_from_windows_micros(micros: i64) -> i64 {
    (micros - WINDOWS_EPOCH_OFFSET_MICROS) / MICROS_PER_MILLI
}

pub(crate) fn cache_guid() -> String {
    let mut guid = [0; 16];
    rand::rng().fill_bytes(&mut guid);
    STANDARD.encode(guid)
}

pub(crate) fn uuid() -> String {
    let mut bytes = [0; 16];
    rand::rng().fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = rikka_types::hex::encode(bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

pub(crate) fn padding() -> String {
    let mut rng = rand::rng();
    (0..PADDING)
        .map(|_| char::from(b' ' + (rng.next_u32() % 95) as u8))
        .collect()
}

pub(crate) fn unknown_fields(message: &[u8], known: impl Fn(u32) -> bool) -> Vec<u8> {
    let mut unknown = Vec::new();
    let mut rest = message;
    while !rest.is_empty() {
        let Some((key, after_key)) = varint(rest) else {
            break;
        };
        let Some(after_value) = skip(after_key, (key & 7) as u8) else {
            break;
        };
        let field = (key >> 3) as u32;
        if !known(field) {
            unknown.extend_from_slice(&rest[..rest.len() - after_value.len()]);
        }
        rest = after_value;
    }
    unknown
}

fn varint(bytes: &[u8]) -> Option<(u64, &[u8])> {
    let mut value = 0_u64;
    for (index, byte) in bytes.iter().enumerate().take(10) {
        value |= u64::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            return Some((value, &bytes[index + 1..]));
        }
    }
    None
}

fn skip(bytes: &[u8], wire_type: u8) -> Option<&[u8]> {
    match wire_type {
        0 => varint(bytes).map(|(_, rest)| rest),
        1 => bytes.get(8..),
        2 => {
            let (length, rest) = varint(bytes)?;
            rest.get(usize::try_from(length).ok()?..)
        }
        5 => bytes.get(4..),
        _ => None,
    }
}
