use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use rikka_model_core::Error;
use rikka_model_core::path::relative_parts;

const DATA: &str = "data:";
const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";

const BASE64: &str = ";base64";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Uri {
    Data {
        mime: Option<String>,
        bytes: Vec<u8>,
    },
    File(String),
    Remote(String),
}

pub(crate) fn parse(uri: &str) -> Result<Uri, Error> {
    if let Some(rest) = uri.strip_prefix(DATA) {
        return data(uri, rest);
    }
    match scheme(uri) {
        Some(_) => Ok(Uri::Remote(uri.to_owned())),
        None => Ok(Uri::File(decode(uri))),
    }
}

fn data(uri: &str, rest: &str) -> Result<Uri, Error> {
    let (header, payload) = rest
        .split_once(',')
        .ok_or_else(|| unreadable(uri, "data URI has no comma"))?;
    let (media, base64) = match header.strip_suffix(BASE64) {
        Some(media) => (media, true),
        None => (header, false),
    };
    let bytes = match base64 {
        true => STANDARD
            .decode(payload.trim())
            .map_err(|error| unreadable(uri, &error.to_string()))?,
        false => decode(payload).into_bytes(),
    };
    Ok(Uri::Data {
        mime: (!media.is_empty()).then(|| media.to_owned()),
        bytes,
    })
}

#[must_use]
pub(crate) fn embed(mime: &str, bytes: &[u8]) -> String {
    format!("{DATA}{mime}{BASE64},{}", STANDARD.encode(bytes))
}

#[must_use]
pub(crate) fn relative(name: &str) -> String {
    let parts = relative_parts(name).unwrap_or_default();
    let path = match parts.is_empty() {
        true => "resource".to_owned(),
        false => parts.join("/"),
    };
    path.split('/')
        .map(encode_part)
        .collect::<Vec<_>>()
        .join("/")
}

fn scheme(uri: &str) -> Option<&str> {
    let (scheme, rest) = uri.split_once(':')?;
    let named = !scheme.is_empty()
        && scheme.starts_with(|first: char| first.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '+' | '-' | '.'));
    (named && rest.starts_with("//")).then_some(scheme)
}

fn decode(uri: &str) -> String {
    let bytes = uri.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match (bytes[at], hex(bytes.get(at + 1)), hex(bytes.get(at + 2))) {
            (b'%', Some(high), Some(low)) => {
                decoded.push(high * 16 + low);
                at += 3;
            }
            (byte, _, _) => {
                decoded.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn encode_part(part: &str) -> String {
    let mut encoded = String::with_capacity(part.len());
    for byte in part.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push('%');
                encoded.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
                encoded.push(char::from(HEX_DIGITS[usize::from(byte & 0xF)]));
            }
        }
    }
    encoded
}

fn hex(byte: Option<&u8>) -> Option<u8> {
    match byte? {
        digit @ b'0'..=b'9' => Some(digit - b'0'),
        letter @ b'a'..=b'f' => Some(letter - b'a' + 10),
        letter @ b'A'..=b'F' => Some(letter - b'A' + 10),
        _ => None,
    }
}

fn unreadable(uri: &str, reason: &str) -> Error {
    Error::Read {
        uri: uri.chars().take(64).collect(),
        reason: reason.to_owned(),
    }
}
