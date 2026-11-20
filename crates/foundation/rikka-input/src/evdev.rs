use rikka_types::{Key, PointerButton, SCROLL_LINE};

pub const BTN_LEFT: u32 = 0x110;
pub const BTN_RIGHT: u32 = 0x111;
pub const BTN_MIDDLE: u32 = 0x112;
pub const BTN_SIDE: u32 = 0x113;
pub const BTN_EXTRA: u32 = 0x114;
pub const BTN_FORWARD: u32 = 0x115;
pub const BTN_BACK: u32 = 0x116;
pub const BTN_STYLUS3: u32 = 0x149;
pub const BTN_STYLUS: u32 = 0x14b;
pub const BTN_STYLUS2: u32 = 0x14c;

pub const VALUE120_PER_DETENT: i32 = 120;

#[must_use]
pub fn pointer_button(code: u32) -> PointerButton {
    match code {
        BTN_LEFT => PointerButton::Primary,
        BTN_RIGHT => PointerButton::Secondary,
        BTN_MIDDLE => PointerButton::Middle,
        BTN_SIDE | BTN_BACK => PointerButton::Back,
        BTN_EXTRA | BTN_FORWARD => PointerButton::Forward,
        other => PointerButton::Other(other as u16),
    }
}

#[must_use]
pub fn tool_button(code: u32) -> PointerButton {
    match code {
        BTN_STYLUS => PointerButton::Secondary,
        BTN_STYLUS2 => PointerButton::Middle,
        BTN_STYLUS3 => PointerButton::Back,
        other => pointer_button(other),
    }
}

#[must_use]
pub fn button_code(button: PointerButton) -> u32 {
    match button {
        PointerButton::Primary => BTN_LEFT,
        PointerButton::Secondary => BTN_RIGHT,
        PointerButton::Middle => BTN_MIDDLE,
        PointerButton::Back => BTN_SIDE,
        PointerButton::Forward => BTN_EXTRA,
        PointerButton::Other(other) => u32::from(other),
    }
}

pub const XKB_OFFSET: u32 = 8;

pub const KEY_LEFTCTRL: u32 = 29;
pub const KEY_LEFTSHIFT: u32 = 42;
pub const KEY_RIGHTSHIFT: u32 = 54;
pub const KEY_LEFTALT: u32 = 56;
pub const KEY_RIGHTCTRL: u32 = 97;
pub const KEY_RIGHTALT: u32 = 100;
pub const KEY_LEFTMETA: u32 = 125;
pub const KEY_RIGHTMETA: u32 = 126;

#[must_use]
pub fn key_code(key: Key) -> Option<u32> {
    let code = match key {
        Key::Escape => 1,
        Key::Backspace => 14,
        Key::Tab => 15,
        Key::Enter => 28,
        Key::Space => 57,
        Key::Home => 102,
        Key::ArrowUp => 103,
        Key::PageUp => 104,
        Key::ArrowLeft => 105,
        Key::ArrowRight => 106,
        Key::End => 107,
        Key::ArrowDown => 108,
        Key::PageDown => 109,
        Key::Delete => 111,
        Key::Insert => 110,
        Key::PrintScreen => 99,
        Key::Menu => 127,
        Key::Undo => 131,
        Key::Copy => 133,
        Key::Paste => 135,
        Key::Find => 136,
        Key::Cut => 137,
        Key::BrowserBack => 158,
        Key::BrowserForward => 159,
        Key::BrowserHome => 172,
        Key::BrowserRefresh => 173,
        Key::Redo => 182,
        Key::F(number @ 1..=10) => 58 + u32::from(number),
        Key::F(number @ 11..=12) => 76 + u32::from(number),
        Key::F(number @ 13..=24) => 170 + u32::from(number),
        Key::ZenkakuHankaku => 85,
        Key::Convert => 92,
        Key::KanaMode => 93,
        Key::NonConvert => 94,
        Key::HangulMode => 122,
        Key::HanjaMode => 123,
        Key::ShiftLeft => KEY_LEFTSHIFT,
        Key::ShiftRight => KEY_RIGHTSHIFT,
        Key::ControlLeft => KEY_LEFTCTRL,
        Key::ControlRight => KEY_RIGHTCTRL,
        Key::AltLeft => KEY_LEFTALT,
        Key::AltRight => KEY_RIGHTALT,
        Key::MetaLeft => KEY_LEFTMETA,
        Key::MetaRight => KEY_RIGHTMETA,
        Key::Eisu => return None,
        Key::F(_) | Key::Unidentified => return None,
        Key::Char(character) => return character_code(character),
    };
    Some(code)
}

fn character_code(character: char) -> Option<u32> {
    const ROWS: [(u32, &str); 4] = [
        (2, "1234567890-="),
        (16, "qwertyuiop[]"),
        (30, "asdfghjkl;'`"),
        (43, "\\zxcvbnm,./"),
    ];
    let lowered = character.to_ascii_lowercase();
    ROWS.into_iter().find_map(|(first, row)| {
        let place = row.chars().position(|key| key == lowered)?;
        Some(first + place as u32)
    })
}

#[must_use]
pub fn value120_to_pixels(value120: i32) -> f32 {
    value120 as f32 / VALUE120_PER_DETENT as f32 * SCROLL_LINE
}
