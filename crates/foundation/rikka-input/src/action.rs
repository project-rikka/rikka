use std::fmt;
use std::str::FromStr;

use rikka_types::{Key, Modifiers};

use crate::error::{Error, Result};
use crate::single::exactly_one;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    modifiers: Modifiers,
    key: Key,
}

impl Chord {
    #[must_use]
    pub fn new(modifiers: Modifiers, key: Key) -> Self {
        Self {
            modifiers,
            key: key.to_lowercase(),
        }
    }

    #[must_use]
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    #[must_use]
    pub fn key(&self) -> Key {
        self.key
    }

    #[must_use]
    pub fn matches(&self, modifiers: Modifiers, key: Key) -> bool {
        self.modifiers == modifiers && self.key == key.to_lowercase()
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let modifiers = modifier_text(self.modifiers);
        if !modifiers.is_empty() {
            write!(f, "{modifiers}+")?;
        }
        match self.key {
            Key::Char(character) => f.write_str(&display_case(character)),
            Key::F(number) => write!(f, "F{number}"),
            named => f.write_str(key_name(named).unwrap_or(UNIDENTIFIED)),
        }
    }
}

impl FromStr for Chord {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        let text = text.trim();
        if text.is_empty() {
            return Err(Error::EmptyChord);
        }

        let (modifier_text, key_text) = match text.rsplit_once('+') {
            Some((head, "")) => (head.strip_suffix('+').unwrap_or(head), "+"),
            Some((head, key)) => (head, key),
            None => ("", text),
        };

        let parsed = parse_modifiers(modifier_text)?;
        let key_text = key_text.trim();
        let key = named_key(key_text).ok_or_else(|| Error::UnknownKey(key_text.to_owned()))?;
        Ok(Self::new(parsed, key))
    }
}

#[must_use]
pub(crate) fn modifier_text(modifiers: Modifiers) -> String {
    [
        (modifiers.control, "Ctrl"),
        (modifiers.alt, "Alt"),
        (modifiers.logo, "Super"),
        (modifiers.shift, "Shift"),
    ]
    .into_iter()
    .filter_map(|(pressed, name)| pressed.then_some(name))
    .collect::<Vec<_>>()
    .join("+")
}

pub fn parse_modifiers(text: &str) -> Result<Modifiers> {
    let mut parsed = Modifiers::NONE;
    for part in text.split('+') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let modifier =
            named_modifier(part).ok_or_else(|| Error::UnknownModifier(part.to_owned()))?;
        if (parsed | modifier) == parsed {
            return Err(Error::RepeatedModifier(part.to_owned()));
        }
        parsed |= modifier;
    }
    Ok(parsed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Binding<A> {
    pub chord: Chord,
    pub action: A,
}

#[derive(Clone, Debug)]
pub struct ActionMap<A> {
    bindings: Vec<Binding<A>>,
}

impl<A> ActionMap<A> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    #[must_use]
    pub fn bind(mut self, chord: Chord, action: A) -> Self {
        match self
            .bindings
            .iter_mut()
            .find(|binding| binding.chord == chord)
        {
            Some(binding) => binding.action = action,
            None => self.bindings.push(Binding { chord, action }),
        }
        self
    }

    #[must_use]
    pub fn resolve(&self, modifiers: Modifiers, key: Key) -> Option<&A> {
        self.bindings
            .iter()
            .find(|binding| binding.chord.matches(modifiers, key))
            .map(|binding| &binding.action)
    }

    #[must_use]
    pub fn has_binding_with_modifiers(&self, modifiers: Modifiers) -> bool {
        self.bindings
            .iter()
            .any(|binding| binding.chord.modifiers() == modifiers)
    }

    #[must_use]
    pub fn chord_for(&self, action: &A) -> Option<Chord>
    where
        A: PartialEq,
    {
        self.bindings
            .iter()
            .find(|binding| binding.action == *action)
            .map(|binding| binding.chord)
    }
}

impl<A> Default for ActionMap<A> {
    fn default() -> Self {
        Self::new()
    }
}

const UNIDENTIFIED: &str = "Unidentified";

fn named_modifier(text: &str) -> Option<Modifiers> {
    if text.eq_ignore_ascii_case("shift") {
        return Some(Modifiers::SHIFT);
    }
    if text.eq_ignore_ascii_case("ctrl") || text.eq_ignore_ascii_case("control") {
        return Some(Modifiers::CONTROL);
    }
    if text.eq_ignore_ascii_case("alt") || text.eq_ignore_ascii_case("option") {
        return Some(Modifiers::ALT);
    }
    if text.eq_ignore_ascii_case("super")
        || text.eq_ignore_ascii_case("logo")
        || text.eq_ignore_ascii_case("meta")
        || text.eq_ignore_ascii_case("win")
        || text.eq_ignore_ascii_case("cmd")
    {
        return Some(Modifiers::LOGO);
    }
    None
}

fn key_name(key: Key) -> Option<&'static str> {
    Some(match key {
        Key::Enter => "Enter",
        Key::Escape => "Escape",
        Key::Tab => "Tab",
        Key::Backspace => "Backspace",
        Key::Delete => "Delete",
        Key::Insert => "Insert",
        Key::Space => "Space",
        Key::ArrowUp => "Up",
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::Home => "Home",
        Key::End => "End",
        Key::PageUp => "PageUp",
        Key::PageDown => "PageDown",
        Key::PrintScreen => "PrtSc",
        Key::Menu => "Menu",
        Key::BrowserBack => "Back",
        Key::BrowserForward => "Forward",
        Key::BrowserRefresh => "Refresh",
        Key::BrowserHome => "HomePage",
        Key::Copy => "Copy",
        Key::Cut => "Cut",
        Key::Paste => "Paste",
        Key::Undo => "Undo",
        Key::Redo => "Redo",
        Key::Find => "Find",
        Key::Convert => "Henkan",
        Key::NonConvert => "Muhenkan",
        Key::ZenkakuHankaku => "ZenkakuHankaku",
        Key::KanaMode => "Kana",
        Key::Eisu => "Eisu",
        Key::HangulMode => "Hangul",
        Key::HanjaMode => "Hanja",
        Key::ShiftLeft => "ShiftLeft",
        Key::ShiftRight => "ShiftRight",
        Key::ControlLeft => "ControlLeft",
        Key::ControlRight => "ControlRight",
        Key::AltLeft => "AltLeft",
        Key::AltRight => "AltRight",
        Key::MetaLeft => "MetaLeft",
        Key::MetaRight => "MetaRight",
        Key::Unidentified => UNIDENTIFIED,
        Key::Char(_) | Key::F(_) => return None,
    })
}

fn named_key(text: &str) -> Option<Key> {
    if let Some(key) = function_key(text) {
        return Some(key);
    }
    let named = [
        Key::Enter,
        Key::Escape,
        Key::Tab,
        Key::Backspace,
        Key::Delete,
        Key::Insert,
        Key::Space,
        Key::ArrowUp,
        Key::ArrowDown,
        Key::ArrowLeft,
        Key::ArrowRight,
        Key::Home,
        Key::End,
        Key::PageUp,
        Key::PageDown,
        Key::PrintScreen,
        Key::Menu,
        Key::BrowserBack,
        Key::BrowserForward,
        Key::BrowserRefresh,
        Key::BrowserHome,
        Key::Copy,
        Key::Cut,
        Key::Paste,
        Key::Undo,
        Key::Redo,
        Key::Find,
        Key::Convert,
        Key::NonConvert,
        Key::ZenkakuHankaku,
        Key::KanaMode,
        Key::Eisu,
        Key::HangulMode,
        Key::HanjaMode,
        Key::ShiftLeft,
        Key::ShiftRight,
        Key::ControlLeft,
        Key::ControlRight,
        Key::AltLeft,
        Key::AltRight,
        Key::MetaLeft,
        Key::MetaRight,
        Key::Unidentified,
    ]
    .into_iter()
    .find(|key| key_name(*key).is_some_and(|name| name.eq_ignore_ascii_case(text)));
    if let Some(key) = named {
        return Some(key);
    }
    let aliased = match text.to_ascii_lowercase().as_str() {
        "arrowup" => Key::ArrowUp,
        "arrowdown" => Key::ArrowDown,
        "arrowleft" => Key::ArrowLeft,
        "arrowright" => Key::ArrowRight,
        "return" => Key::Enter,
        "esc" => Key::Escape,
        "print" | "printscreen" => Key::PrintScreen,
        "convert" => Key::Convert,
        "nonconvert" => Key::NonConvert,
        "zenkaku" | "hankaku" => Key::ZenkakuHankaku,
        "kanamode" | "hiragana" | "katakana" => Key::KanaMode,
        "hangulmode" => Key::HangulMode,
        "hanjamode" => Key::HanjaMode,
        "browserback" => Key::BrowserBack,
        "browserforward" => Key::BrowserForward,
        "browserrefresh" | "reload" => Key::BrowserRefresh,
        "browserhome" => Key::BrowserHome,
        _ => return exactly_one(text.chars()).map(Key::Char),
    };
    Some(aliased)
}

fn function_key(text: &str) -> Option<Key> {
    let digits = text.strip_prefix(['F', 'f'])?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let number: u8 = digits.parse().ok()?;
    (1..=24).contains(&number).then_some(Key::F(number))
}

fn display_case(character: char) -> String {
    match exactly_one(character.to_uppercase()) {
        Some(raised) => raised.to_string(),
        None => character.to_string(),
    }
}
