use rikka_secrets_protocol::names::SCHEMA_ATTRIBUTE;

use crate::entry::Attributes;

const PREFIX: &str = "rikka:";
const PART_PREFIX: &str = "rikka-part:";
const EVERYTHING: &str = "rikka:*";

pub(super) fn of(attributes: &Attributes) -> String {
    let schema = attributes.get(SCHEMA_ATTRIBUTE).map_or("", String::as_str);
    let pairs: Vec<String> = attributes
        .iter()
        .filter(|(name, _)| name.as_str() != SCHEMA_ATTRIBUTE)
        .map(|(name, value)| format!("{}={}", encode(name), encode(value)))
        .collect();
    format!("{PREFIX}{}?{}", encode(schema), pairs.join("&"))
}

pub(super) fn filter(attributes: &Attributes) -> String {
    match attributes.get(SCHEMA_ATTRIBUTE) {
        Some(schema) => format!("{PREFIX}{}?*", encode(schema)),
        None => EVERYTHING.to_owned(),
    }
}

pub(super) fn everything() -> &'static str {
    EVERYTHING
}

pub(super) fn attributes(target: &str) -> Option<Attributes> {
    let (schema, pairs) = target.strip_prefix(PREFIX)?.split_once('?')?;
    let mut attributes = Attributes::new();
    let schema = decode(schema)?;
    if !schema.is_empty() {
        attributes.insert(SCHEMA_ATTRIBUTE.to_owned(), schema);
    }
    for pair in pairs.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=')?;
        attributes.insert(decode(name)?, decode(value)?);
    }
    Some(attributes)
}

pub(super) fn part(target: &str, index: usize) -> String {
    format!("{PART_PREFIX}{index}:{target}")
}

fn encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            true => encoded.push(char::from(byte)),
            false => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn decode(text: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut rest = text.bytes();
    while let Some(byte) = rest.next() {
        match byte {
            b'%' => {
                let high = char::from(rest.next()?).to_digit(16)?;
                let low = char::from(rest.next()?).to_digit(16)?;
                bytes.push(u8::try_from(high * 16 + low).ok()?);
            }
            _ => bytes.push(byte),
        }
    }
    String::from_utf8(bytes).ok()
}
