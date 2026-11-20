use std::collections::HashMap;
use std::fmt;

use rikka_input::{Chord, evdev, xkb};
use rikka_types::{Key, Modifiers};

use crate::cap::Cap;
use crate::error::{Error, Result};

const MODIFIERS: [(u32, &str, &str, u32); 8] = [
    (evdev::KEY_LEFTSHIFT, "Shift_L", "Shift", xkb::SHIFT),
    (evdev::KEY_RIGHTSHIFT, "Shift_R", "Shift", xkb::SHIFT),
    (evdev::KEY_LEFTCTRL, "Control_L", "Control", xkb::CONTROL),
    (evdev::KEY_RIGHTCTRL, "Control_R", "Control", xkb::CONTROL),
    (evdev::KEY_LEFTALT, "Alt_L", "Mod1", xkb::MOD1),
    (evdev::KEY_RIGHTALT, "Alt_R", "Mod1", xkb::MOD1),
    (evdev::KEY_LEFTMETA, "Super_L", "Mod4", xkb::MOD4),
    (evdev::KEY_RIGHTMETA, "Super_R", "Mod4", xkb::MOD4),
];

pub(crate) fn modifier_bit(code: u32) -> Option<u32> {
    MODIFIERS
        .into_iter()
        .find(|(modifier, ..)| *modifier == code)
        .map(|(.., bit)| bit)
}

pub(crate) fn modifier_codes(modifiers: Modifiers) -> Vec<u32> {
    [
        (modifiers.control, evdev::KEY_LEFTCTRL),
        (modifiers.shift, evdev::KEY_LEFTSHIFT),
        (modifiers.alt, evdev::KEY_LEFTALT),
        (modifiers.logo, evdev::KEY_LEFTMETA),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .map(|(_, code)| code)
    .collect()
}

pub(crate) fn for_chord(chord: Chord) -> Result<(String, u32)> {
    let key = chord.key();
    let code = evdev::key_code(key).ok_or(Error::Unsupported(key))?;
    Ok((compile([(code, levels(key)?)]), code))
}

pub(crate) fn for_caps(caps: &[Cap]) -> Result<String> {
    let mut keys = Vec::with_capacity(caps.len());
    for cap in caps {
        keys.push((cap_code(*cap)?, cap_levels(*cap)?));
    }
    Ok(compile(keys))
}

pub(crate) fn cap_code(cap: Cap) -> Result<u32> {
    cap.code().ok_or(Error::Unsupported(cap.key()))
}

pub(crate) fn empty() -> String {
    compile([])
}

const FIRST_KEYCODE: u32 = 1;
const LAST_KEYCODE: u32 = 255 - evdev::XKB_OFFSET;

pub(crate) struct Segment {
    pub(crate) keymap: String,
    pub(crate) codes: Vec<u32>,
}

pub(crate) fn for_text(text: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut builder = SegmentBuilder::default();

    for character in text.chars() {
        if !builder.try_push(character) {
            segments.push(builder.finish());
            builder = SegmentBuilder::default();
            builder.try_push(character);
        }
    }

    if !builder.is_empty() {
        segments.push(builder.finish());
    }
    segments
}

#[derive(Default)]
struct SegmentBuilder {
    keycodes: HashMap<char, u32>,
    codes: Vec<u32>,
}

impl SegmentBuilder {
    fn try_push(&mut self, character: char) -> bool {
        if let Some(code) = self.keycodes.get(&character) {
            self.codes.push(*code);
            return true;
        }
        let Some(code) = free_keycode(self.keycodes.len()) else {
            return false;
        };
        self.keycodes.insert(character, code);
        self.codes.push(code);
        true
    }

    fn is_empty(&self) -> bool {
        self.codes.is_empty()
    }

    fn finish(self) -> Segment {
        let mut keys: Vec<(u32, String)> = self
            .keycodes
            .into_iter()
            .map(|(character, code)| (code, keysym_name(character)))
            .collect();
        keys.sort_unstable_by_key(|(code, _)| *code);

        Segment {
            keymap: compile(keys),
            codes: self.codes,
        }
    }
}

fn free_keycode(index: usize) -> Option<u32> {
    (FIRST_KEYCODE..=LAST_KEYCODE)
        .filter(|code| !MODIFIERS.iter().any(|(modifier, ..)| modifier == code))
        .nth(index)
}

fn keysym_name(character: char) -> String {
    match character {
        '\n' => "Return".to_owned(),
        '\t' => "Tab".to_owned(),
        character => unicode(character),
    }
}

fn compile(keys: impl IntoIterator<Item = (u32, String)>) -> String {
    let declared: Vec<(u32, String, Option<&str>)> = MODIFIERS
        .into_iter()
        .map(|(code, keysym, modifier, _)| (code, keysym.to_owned(), Some(modifier)))
        .chain(
            keys.into_iter()
                .filter(|(code, _)| modifier_bit(*code).is_none())
                .map(|(code, levels)| (code, levels, None)),
        )
        .map(|(code, levels, modifier)| (code + evdev::XKB_OFFSET, levels, modifier))
        .collect();

    fmt::from_fn(|out| {
        out.write_str(
            "xkb_keymap {\nxkb_keycodes \"rikka\" {\n\tminimum = 8;\n\tmaximum = 255;\n",
        )?;
        for (keycode, ..) in &declared {
            writeln!(out, "\t<K{keycode}> = {keycode};")?;
        }
        out.write_str(
            "};\n\
             xkb_types \"rikka\" { include \"complete\" };\n\
             xkb_compatibility \"rikka\" { include \"complete\" };\n\
             xkb_symbols \"rikka\" {\n",
        )?;
        for (keycode, levels, _) in &declared {
            writeln!(out, "\tkey <K{keycode}> {{ [ {levels} ] }};")?;
        }
        for (keycode, _, modifier) in &declared {
            if let Some(modifier) = modifier {
                writeln!(out, "\tmodifier_map {modifier} {{ <K{keycode}> }};")?;
            }
        }
        out.write_str("};\n};\n")
    })
    .to_string()
}

fn cap_levels(cap: Cap) -> Result<String> {
    match cap {
        Cap::Named(key) => levels(key),
        Cap::Levels(plain, shifted) if plain == shifted => Ok(unicode(plain)),
        Cap::Levels(plain, shifted) => Ok(format!("{}, {}", unicode(plain), unicode(shifted))),
    }
}

fn levels(key: Key) -> Result<String> {
    if let Some(keysym) = xkb::keysym(key) {
        let name = keysym
            .name()
            .and_then(|name| name.strip_prefix("XK_"))
            .ok_or(Error::Unsupported(key))?;
        return Ok(name.to_owned());
    }
    let Key::Char(character) = key else {
        return Err(Error::Unsupported(key));
    };
    let lower = character.to_ascii_lowercase();
    let upper = character.to_ascii_uppercase();
    Ok(match lower == upper {
        true => unicode(lower),
        false => format!("{}, {}", unicode(lower), unicode(upper)),
    })
}

fn unicode(character: char) -> String {
    format!("U{:04X}", character as u32)
}
