use rikka_app::prelude::Key;

const NAMED: [(Key, &str); 22] = [
    (Key::Space, "Space"),
    (Key::Tab, "Tab"),
    (Key::Insert, "Insert"),
    (Key::Delete, "Delete"),
    (Key::Home, "Home"),
    (Key::End, "End"),
    (Key::PageUp, "PageUp"),
    (Key::PageDown, "PageDown"),
    (Key::ArrowUp, "ArrowUp"),
    (Key::ArrowDown, "ArrowDown"),
    (Key::ArrowLeft, "ArrowLeft"),
    (Key::ArrowRight, "ArrowRight"),
    (Key::Menu, "Menu"),
    (Key::PrintScreen, "PrintScreen"),
    (Key::ShiftLeft, "ShiftLeft"),
    (Key::ShiftRight, "ShiftRight"),
    (Key::ControlLeft, "ControlLeft"),
    (Key::ControlRight, "ControlRight"),
    (Key::AltLeft, "AltLeft"),
    (Key::AltRight, "AltRight"),
    (Key::MetaLeft, "MetaLeft"),
    (Key::MetaRight, "MetaRight"),
];

pub(crate) fn encode(key: Key) -> Option<String> {
    match key {
        Key::Char(character) if !character.is_control() && !character.is_whitespace() => {
            Some(format!("Char:{}", character.to_lowercase()))
        }
        Key::F(number) => Some(format!("F{number}")),
        other => NAMED
            .iter()
            .find(|(named, _)| *named == other)
            .map(|(_, name)| (*name).to_owned()),
    }
}

pub(crate) fn decode(encoded: &str) -> Option<Key> {
    if let Some(character) = encoded.strip_prefix("Char:") {
        let mut characters = character.chars();
        return match (characters.next(), characters.next()) {
            (Some(single), None) => Some(Key::Char(single)),
            _ => None,
        };
    }
    if let Some(number) = encoded.strip_prefix('F')
        && let Ok(number) = number.parse::<u8>()
    {
        return Some(Key::F(number));
    }
    NAMED
        .iter()
        .find(|(_, name)| *name == encoded)
        .map(|(key, _)| *key)
}

pub(crate) fn label(key: Key) -> String {
    match key {
        Key::Char(character) => character.to_uppercase().collect(),
        Key::F(number) => format!("F{number}"),
        other => encode(other).unwrap_or_default(),
    }
}

pub(crate) fn same(pressed: Key, binding: Key) -> bool {
    pressed.to_lowercase() == binding.to_lowercase()
}
