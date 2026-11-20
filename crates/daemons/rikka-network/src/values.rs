use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};

use rikka_bus::Properties;

pub(crate) type RawSettings = HashMap<String, HashMap<String, OwnedValue>>;

pub(crate) type Settings = HashMap<String, Properties>;

pub(crate) type Draft = HashMap<String, HashMap<String, Value<'static>>>;

pub(crate) fn settings(raw: RawSettings) -> Settings {
    raw.into_iter()
        .map(|(name, properties)| (name, Properties::from(properties)))
        .collect()
}

pub(crate) fn draft(settings: &Settings) -> Draft {
    settings
        .iter()
        .map(|(name, properties)| {
            let properties = properties
                .iter()
                .filter_map(|(key, value)| Some((key.to_owned(), value.try_clone().ok()?.into())))
                .collect();
            (name.clone(), properties)
        })
        .collect()
}

pub(crate) fn put(draft: &mut Draft, setting: &str, key: &str, value: impl Into<Value<'static>>) {
    draft
        .entry(setting.to_owned())
        .or_default()
        .insert(key.to_owned(), value.into());
}

pub(crate) fn erase(draft: &mut Draft, setting: &str, key: &str) {
    if let Some(properties) = draft.get_mut(setting) {
        properties.remove(key);
    }
}

pub(crate) fn address_of(bytes: &[u8]) -> Option<String> {
    match bytes.len() {
        4 => Some(std::net::Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]).to_string()),
        16 => {
            let mut octets = [0_u8; 16];
            octets.copy_from_slice(bytes);
            Some(std::net::Ipv6Addr::from(octets).to_string())
        }
        _ => None,
    }
}

pub(crate) fn address_of_word(word: u32) -> String {
    let [a, b, c, d] = word.to_ne_bytes();
    std::net::Ipv4Addr::new(a, b, c, d).to_string()
}

pub(crate) fn word_of_address(address: &str) -> Option<u32> {
    let parsed: std::net::Ipv4Addr = address.parse().ok()?;
    Some(u32::from_ne_bytes(parsed.octets()))
}

pub(crate) fn bytes_of_address(address: &str) -> Option<Vec<u8>> {
    let parsed: std::net::Ipv6Addr = address.parse().ok()?;
    Some(parsed.octets().to_vec())
}
