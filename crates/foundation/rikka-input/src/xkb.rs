#[cfg(unix)]
mod keyboard;
mod session;

use rikka_types::{Key, KeyLocation, Modifiers};
use xkeysym::Keysym;

use crate::single::exactly_one;

#[cfg(unix)]
pub use keyboard::{Keyboard, Keystroke};
pub use session::{KeyRepeat, SessionKeyboard};

pub const SHIFT: u32 = 1 << 0;

pub const LOCK: u32 = 1 << 1;

pub const CONTROL: u32 = 1 << 2;

pub const MOD1: u32 = 1 << 3;

pub const MOD2: u32 = 1 << 4;

pub const MOD3: u32 = 1 << 5;

pub const MOD4: u32 = 1 << 6;

pub const MOD5: u32 = 1 << 7;

#[must_use]
pub fn mask(modifiers: Modifiers) -> u32 {
    [
        (modifiers.shift, SHIFT),
        (modifiers.control, CONTROL),
        (modifiers.alt, MOD1),
        (modifiers.logo, MOD4),
    ]
    .into_iter()
    .filter(|(down, _)| *down)
    .map(|(_, bit)| bit)
    .sum()
}

#[must_use]
pub fn key(keysym: Keysym) -> Key {
    if let Some(named) = named_key(keysym) {
        return named;
    }
    character(keysym.key_char())
}

#[must_use]
pub fn key_with_text(keysym: Keysym, text: Option<&str>) -> Key {
    if let Some(named) = named_key(keysym) {
        return named;
    }
    match character(text.and_then(|text| exactly_one(text.chars()))) {
        Key::Unidentified => character(keysym.key_char()),
        resolved => resolved,
    }
}

#[must_use]
pub fn location(keysym: Keysym) -> KeyLocation {
    match keysym.is_keypad_key() {
        true => KeyLocation::Keypad,
        false => KeyLocation::Standard,
    }
}

fn character(character: Option<char>) -> Key {
    match character {
        Some(character) if !character.is_control() => Key::Char(character),
        _ => Key::Unidentified,
    }
}

fn named_key(keysym: Keysym) -> Option<Key> {
    Some(match keysym {
        Keysym::Return | Keysym::KP_Enter => Key::Enter,
        Keysym::Escape => Key::Escape,
        Keysym::Tab | Keysym::ISO_Left_Tab => Key::Tab,
        Keysym::BackSpace => Key::Backspace,
        Keysym::Delete | Keysym::KP_Delete => Key::Delete,
        Keysym::Insert | Keysym::KP_Insert => Key::Insert,
        Keysym::space | Keysym::KP_Space => Key::Space,
        Keysym::Up | Keysym::KP_Up => Key::ArrowUp,
        Keysym::Down | Keysym::KP_Down => Key::ArrowDown,
        Keysym::Left | Keysym::KP_Left => Key::ArrowLeft,
        Keysym::Right | Keysym::KP_Right => Key::ArrowRight,
        Keysym::Home | Keysym::KP_Home => Key::Home,
        Keysym::End | Keysym::KP_End => Key::End,
        Keysym::Page_Up | Keysym::KP_Page_Up => Key::PageUp,
        Keysym::Page_Down | Keysym::KP_Page_Down => Key::PageDown,
        Keysym::Print => Key::PrintScreen,
        Keysym::Menu => Key::Menu,
        Keysym::XF86_Back => Key::BrowserBack,
        Keysym::XF86_Forward => Key::BrowserForward,
        Keysym::XF86_Reload | Keysym::XF86_Refresh => Key::BrowserRefresh,
        Keysym::XF86_HomePage => Key::BrowserHome,
        Keysym::XF86_Copy => Key::Copy,
        Keysym::XF86_Cut => Key::Cut,
        Keysym::XF86_Paste => Key::Paste,
        Keysym::Undo => Key::Undo,
        Keysym::Redo => Key::Redo,
        Keysym::Find => Key::Find,
        numbered if numbered.is_function_key() => Key::F(function_number(numbered)),
        Keysym::Henkan => Key::Convert,
        Keysym::Muhenkan => Key::NonConvert,
        Keysym::Zenkaku_Hankaku | Keysym::Zenkaku | Keysym::Hankaku => Key::ZenkakuHankaku,
        Keysym::Hiragana_Katakana | Keysym::Kana_Shift | Keysym::Kana_Lock => Key::KanaMode,
        Keysym::Eisu_toggle | Keysym::Eisu_Shift => Key::Eisu,
        Keysym::Hangul => Key::HangulMode,
        Keysym::Hangul_Hanja => Key::HanjaMode,
        Keysym::Shift_L => Key::ShiftLeft,
        Keysym::Shift_R => Key::ShiftRight,
        Keysym::Control_L => Key::ControlLeft,
        Keysym::Control_R => Key::ControlRight,
        Keysym::Alt_L | Keysym::Meta_L => Key::AltLeft,
        Keysym::Alt_R | Keysym::Meta_R | Keysym::ISO_Level3_Shift => Key::AltRight,
        Keysym::Super_L | Keysym::Hyper_L => Key::MetaLeft,
        Keysym::Super_R | Keysym::Hyper_R => Key::MetaRight,
        _ => return None,
    })
}

fn function_number(keysym: Keysym) -> u8 {
    (keysym.raw() - Keysym::F1.raw() + 1) as u8
}

fn function_keysym(number: u8) -> Option<Keysym> {
    let numbered = Keysym::new(Keysym::F1.raw() + u32::from(number.checked_sub(1)?));
    numbered.is_function_key().then_some(numbered)
}

#[must_use]
pub fn keysym(key: Key) -> Option<Keysym> {
    Some(match key {
        Key::Enter => Keysym::Return,
        Key::Escape => Keysym::Escape,
        Key::Tab => Keysym::Tab,
        Key::Backspace => Keysym::BackSpace,
        Key::Delete => Keysym::Delete,
        Key::Insert => Keysym::Insert,
        Key::Space => Keysym::space,
        Key::ArrowUp => Keysym::Up,
        Key::ArrowDown => Keysym::Down,
        Key::ArrowLeft => Keysym::Left,
        Key::ArrowRight => Keysym::Right,
        Key::Home => Keysym::Home,
        Key::End => Keysym::End,
        Key::PageUp => Keysym::Page_Up,
        Key::PageDown => Keysym::Page_Down,
        Key::PrintScreen => Keysym::Print,
        Key::Menu => Keysym::Menu,
        Key::BrowserBack => Keysym::XF86_Back,
        Key::BrowserForward => Keysym::XF86_Forward,
        Key::BrowserRefresh => Keysym::XF86_Reload,
        Key::BrowserHome => Keysym::XF86_HomePage,
        Key::Copy => Keysym::XF86_Copy,
        Key::Cut => Keysym::XF86_Cut,
        Key::Paste => Keysym::XF86_Paste,
        Key::Undo => Keysym::Undo,
        Key::Redo => Keysym::Redo,
        Key::Find => Keysym::Find,
        Key::F(number) => return function_keysym(number),
        Key::Convert => Keysym::Henkan,
        Key::NonConvert => Keysym::Muhenkan,
        Key::ZenkakuHankaku => Keysym::Zenkaku_Hankaku,
        Key::KanaMode => Keysym::Hiragana_Katakana,
        Key::Eisu => Keysym::Eisu_toggle,
        Key::HangulMode => Keysym::Hangul,
        Key::HanjaMode => Keysym::Hangul_Hanja,
        Key::ShiftLeft => Keysym::Shift_L,
        Key::ShiftRight => Keysym::Shift_R,
        Key::ControlLeft => Keysym::Control_L,
        Key::ControlRight => Keysym::Control_R,
        Key::AltLeft => Keysym::Alt_L,
        Key::AltRight => Keysym::Alt_R,
        Key::MetaLeft => Keysym::Super_L,
        Key::MetaRight => Keysym::Super_R,
        Key::Char(_) | Key::Unidentified => return None,
    })
}
#[must_use]
pub fn keysym_at(key: Key, location: KeyLocation) -> Option<Keysym> {
    if location == KeyLocation::Keypad
        && let Some(keypad) = keypad_keysym(key)
    {
        return Some(keypad);
    }
    match key {
        Key::Char(character) => Some(Keysym::from_char(character)),
        named => keysym(named),
    }
}

fn keypad_keysym(key: Key) -> Option<Keysym> {
    Some(match key {
        Key::Enter => Keysym::KP_Enter,
        Key::Char(digit @ '0'..='9') => {
            Keysym::new(Keysym::KP_0.raw() + u32::from(digit) - u32::from('0'))
        }
        Key::Char('.' | ',') => Keysym::KP_Decimal,
        Key::Char('+') => Keysym::KP_Add,
        Key::Char('-') => Keysym::KP_Subtract,
        Key::Char('*') => Keysym::KP_Multiply,
        Key::Char('/') => Keysym::KP_Divide,
        Key::Char('=') => Keysym::KP_Equal,
        _ => return None,
    })
}
