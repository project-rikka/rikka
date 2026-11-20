use rikka_types::{Key, KeyCode, KeyLocation, Point, SCROLL_LINE};

pub const WHEEL_DELTA: i32 = 120;
pub const DEFAULT_WHEEL_LINES: u32 = 3;
pub const WHEEL_PAGE: u32 = u32::MAX;

#[must_use]
pub fn wheel_offset(delta: i32, lines: u32, horizontal: bool) -> Point {
    let lines = match lines {
        WHEEL_PAGE => DEFAULT_WHEEL_LINES,
        lines => lines,
    };
    let notches = delta as f32 / WHEEL_DELTA as f32;
    let amount = notches * lines as f32 / DEFAULT_WHEEL_LINES as f32 * SCROLL_LINE;
    match horizontal {
        true => Point::new(amount, 0.0),
        false => Point::new(0.0, -amount),
    }
}

pub const VK_BACK: u16 = 0x08;
pub const VK_TAB: u16 = 0x09;
pub const VK_RETURN: u16 = 0x0D;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_CONTROL: u16 = 0x11;
pub const VK_MENU: u16 = 0x12;
pub const VK_KANA: u16 = 0x15;
pub const VK_HANJA: u16 = 0x19;
pub const VK_ESCAPE: u16 = 0x1B;
pub const VK_CONVERT: u16 = 0x1C;
pub const VK_NONCONVERT: u16 = 0x1D;
pub const VK_SPACE: u16 = 0x20;
pub const VK_PRIOR: u16 = 0x21;
pub const VK_NEXT: u16 = 0x22;
pub const VK_END: u16 = 0x23;
pub const VK_HOME: u16 = 0x24;
pub const VK_LEFT: u16 = 0x25;
pub const VK_UP: u16 = 0x26;
pub const VK_RIGHT: u16 = 0x27;
pub const VK_DOWN: u16 = 0x28;
pub const VK_SNAPSHOT: u16 = 0x2C;
pub const VK_INSERT: u16 = 0x2D;
pub const VK_DELETE: u16 = 0x2E;
pub const VK_LWIN: u16 = 0x5B;
pub const VK_RWIN: u16 = 0x5C;
pub const VK_APPS: u16 = 0x5D;
pub const VK_F1: u16 = 0x70;
pub const VK_F24: u16 = 0x87;
pub const VK_LSHIFT: u16 = 0xA0;
pub const VK_RSHIFT: u16 = 0xA1;
pub const VK_LCONTROL: u16 = 0xA2;
pub const VK_RCONTROL: u16 = 0xA3;
pub const VK_LMENU: u16 = 0xA4;
pub const VK_RMENU: u16 = 0xA5;
pub const VK_BROWSER_BACK: u16 = 0xA6;
pub const VK_BROWSER_FORWARD: u16 = 0xA7;
pub const VK_BROWSER_REFRESH: u16 = 0xA8;
pub const VK_BROWSER_HOME: u16 = 0xAC;
pub const VK_OEM_ATTN: u16 = 0xF0;
pub const VK_OEM_FINISH: u16 = 0xF1;
pub const VK_OEM_COPY: u16 = 0xF2;
pub const VK_OEM_AUTO: u16 = 0xF3;
pub const VK_OEM_ENLW: u16 = 0xF4;

const SCAN_RIGHT_SHIFT: u16 = 0x36;
const SCAN_HANJA: [u16; 2] = [0x71, 0xF1];
const SCAN_HANGUL: [u16; 2] = [0x72, 0xF2];

#[must_use]
pub fn key_code(scan: u16, extended: bool) -> KeyCode {
    let code = match extended {
        true => extended_code(scan),
        false => plain_code(scan),
    };
    code.map_or(KeyCode::UNKNOWN, KeyCode::from_evdev)
}

fn plain_code(scan: u16) -> Option<u32> {
    Some(match scan {
        0x45 => 119,
        0x54 => 99,
        0x59 => 117,
        0x01..=0x58 => u32::from(scan),
        0x64..=0x6E => u32::from(scan) + 83,
        0x70 => 93,
        0x71 | 0xF1 => 123,
        0x72 | 0xF2 => 122,
        0x73 => 89,
        0x76 => 194,
        0x79 => 92,
        0x7B => 94,
        0x7D => 124,
        0x7E => 95,
        _ => return None,
    })
}

fn extended_code(scan: u16) -> Option<u32> {
    Some(match scan {
        0x10 => 165,
        0x19 => 163,
        0x1C => 96,
        0x1D => 97,
        0x20 => 113,
        0x21 => 140,
        0x22 => 164,
        0x24 => 166,
        0x2E => 114,
        0x30 => 115,
        0x32 => 172,
        0x35 => 98,
        0x37 => 99,
        0x38 => 100,
        0x45 => 69,
        0x46 => 119,
        0x47 => 102,
        0x48 => 103,
        0x49 => 104,
        0x4B => 105,
        0x4D => 106,
        0x4F => 107,
        0x50 => 108,
        0x51 => 109,
        0x52 => 110,
        0x53 => 111,
        0x5B => 125,
        0x5C => 126,
        0x5D => 127,
        0x5E => 116,
        0x5F => 142,
        0x63 => 143,
        0x65 => 217,
        0x66 => 156,
        0x67 => 173,
        0x68 => 128,
        0x69 => 159,
        0x6A => 158,
        0x6B => 157,
        0x6C => 155,
        0x6D => 226,
        _ => return None,
    })
}

#[must_use]
pub fn location(scan: u16, extended: bool) -> KeyLocation {
    let keypad = match extended {
        true => matches!(scan, 0x1C | 0x35),
        false => matches!(scan, 0x37 | 0x47..=0x53 | 0x59 | 0x7E),
    };
    match keypad {
        true => KeyLocation::Keypad,
        false => KeyLocation::Standard,
    }
}

#[must_use]
pub fn named_key(virtual_key: u16, scan: u16, extended: bool) -> Option<Key> {
    Some(match virtual_key {
        VK_BACK => Key::Backspace,
        VK_TAB => Key::Tab,
        VK_RETURN => Key::Enter,
        VK_ESCAPE => Key::Escape,
        VK_SPACE => Key::Space,
        VK_PRIOR => Key::PageUp,
        VK_NEXT => Key::PageDown,
        VK_END => Key::End,
        VK_HOME => Key::Home,
        VK_LEFT => Key::ArrowLeft,
        VK_UP => Key::ArrowUp,
        VK_RIGHT => Key::ArrowRight,
        VK_DOWN => Key::ArrowDown,
        VK_SNAPSHOT => Key::PrintScreen,
        VK_INSERT => Key::Insert,
        VK_DELETE => Key::Delete,
        VK_APPS => Key::Menu,
        VK_F1..=VK_F24 => Key::F((virtual_key - VK_F1 + 1) as u8),
        VK_SHIFT if scan == SCAN_RIGHT_SHIFT => Key::ShiftRight,
        VK_SHIFT | VK_LSHIFT => Key::ShiftLeft,
        VK_RSHIFT => Key::ShiftRight,
        VK_CONTROL if extended => Key::ControlRight,
        VK_CONTROL | VK_LCONTROL => Key::ControlLeft,
        VK_RCONTROL => Key::ControlRight,
        VK_MENU if extended => Key::AltRight,
        VK_MENU | VK_LMENU => Key::AltLeft,
        VK_RMENU => Key::AltRight,
        VK_LWIN => Key::MetaLeft,
        VK_RWIN => Key::MetaRight,
        VK_BROWSER_BACK => Key::BrowserBack,
        VK_BROWSER_FORWARD => Key::BrowserForward,
        VK_BROWSER_REFRESH => Key::BrowserRefresh,
        VK_BROWSER_HOME => Key::BrowserHome,
        VK_CONVERT => Key::Convert,
        VK_NONCONVERT => Key::NonConvert,
        VK_KANA if SCAN_HANGUL.contains(&scan) => Key::HangulMode,
        VK_KANA | VK_OEM_FINISH | VK_OEM_COPY => Key::KanaMode,
        VK_HANJA if SCAN_HANJA.contains(&scan) => Key::HanjaMode,
        VK_OEM_ATTN => Key::Eisu,
        VK_OEM_AUTO | VK_OEM_ENLW => Key::ZenkakuHankaku,
        _ => return None,
    })
}
