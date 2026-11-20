use alacritty_terminal::term::TermMode;
use rikka_types::{Key, KeyLocation, Modifiers};

const CSI: &[u8] = b"\x1b[";
const SS3: &[u8] = b"\x1bO";
const ESC: u8 = 0x1b;
const DEL: u8 = 0x7f;

#[must_use]
pub(crate) fn encode(
    key: Key,
    location: KeyLocation,
    modifiers: Modifiers,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if let Some(bytes) = keypad(key, location, modifiers, mode) {
        return Some(bytes);
    }
    match key {
        Key::Char(character) => typed(character, modifiers),
        Key::Space => typed(' ', modifiers),
        Key::Enter => Some(escaped(newline(mode), modifiers)),
        Key::Escape => Some(escaped(vec![ESC], modifiers)),
        Key::Backspace if modifiers.control => Some(escaped(vec![0x08], modifiers)),
        Key::Backspace => Some(escaped(vec![DEL], modifiers)),
        Key::Tab => Some(tab(modifiers)),
        Key::Insert => Some(tilde(2, modifiers)),
        Key::Delete => Some(tilde(3, modifiers)),
        Key::PageUp => Some(tilde(5, modifiers)),
        Key::PageDown => Some(tilde(6, modifiers)),
        Key::ArrowUp => Some(cursor(b'A', modifiers, mode)),
        Key::ArrowDown => Some(cursor(b'B', modifiers, mode)),
        Key::ArrowRight => Some(cursor(b'C', modifiers, mode)),
        Key::ArrowLeft => Some(cursor(b'D', modifiers, mode)),
        Key::Home => Some(cursor(b'H', modifiers, mode)),
        Key::End => Some(cursor(b'F', modifiers, mode)),
        Key::F(number) => function(number, modifiers),
        _ => None,
    }
}

fn keypad(
    key: Key,
    location: KeyLocation,
    modifiers: Modifiers,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if location != KeyLocation::Keypad
        || !mode.contains(TermMode::APP_KEYPAD)
        || modifiers.control
        || modifiers.shift
    {
        return None;
    }
    let code = match key {
        Key::Enter => b'M',
        Key::Char(character) => keypad_code(character)?,
        _ => return None,
    };
    let mut bytes = SS3.to_vec();
    bytes.push(code);
    Some(escaped(bytes, modifiers))
}

fn keypad_code(character: char) -> Option<u8> {
    Some(match character {
        '0'..='9' => b'p' + (character as u8 - b'0'),
        '.' => b'n',
        ',' => b'l',
        '+' => b'k',
        '-' => b'm',
        '*' => b'j',
        '/' => b'o',
        '=' => b'X',
        _ => return None,
    })
}

fn typed(character: char, modifiers: Modifiers) -> Option<Vec<u8>> {
    let bytes = match modifiers.control {
        true => vec![control(character)?],
        false => {
            let mut room = [0; 4];
            character.encode_utf8(&mut room).as_bytes().to_vec()
        }
    };
    Some(escaped(bytes, modifiers))
}

fn control(character: char) -> Option<u8> {
    let lowered = character.to_ascii_lowercase();
    Some(match lowered {
        'a'..='z' => lowered as u8 - b'a' + 1,
        '@' | '2' | ' ' => 0x00,
        '[' | '3' => 0x1b,
        '\\' | '4' => 0x1c,
        ']' | '5' => 0x1d,
        '^' | '6' => 0x1e,
        '_' | '7' | '/' => 0x1f,
        '8' | '?' => DEL,
        _ => {
            let mut room = [0; 4];
            return Some(*character.encode_utf8(&mut room).as_bytes().first()?);
        }
    })
}

fn newline(mode: TermMode) -> Vec<u8> {
    match mode.contains(TermMode::LINE_FEED_NEW_LINE) {
        true => b"\r\n".to_vec(),
        false => vec![b'\r'],
    }
}

fn tab(modifiers: Modifiers) -> Vec<u8> {
    if !modifiers.shift {
        return escaped(vec![b'\t'], modifiers);
    }
    match parameter(modifiers) {
        2 => b"\x1b[Z".to_vec(),
        _ => sequence(CSI, b'Z', modifiers),
    }
}

fn cursor(final_byte: u8, modifiers: Modifiers, mode: TermMode) -> Vec<u8> {
    let introducer = match mode.contains(TermMode::APP_CURSOR) && parameter(modifiers) == 1 {
        true => SS3,
        false => CSI,
    };
    sequence(introducer, final_byte, modifiers)
}

fn tilde(number: u8, modifiers: Modifiers) -> Vec<u8> {
    let mut bytes = CSI.to_vec();
    bytes.extend_from_slice(number.to_string().as_bytes());
    if parameter(modifiers) != 1 {
        bytes.push(b';');
        bytes.extend_from_slice(parameter(modifiers).to_string().as_bytes());
    }
    bytes.push(b'~');
    bytes
}

fn function(number: u8, modifiers: Modifiers) -> Option<Vec<u8>> {
    if let Some(shifted) = number
        .checked_sub(12)
        .filter(|number| (1..=12).contains(number))
    {
        return function(shifted, modifiers | Modifiers::SHIFT);
    }
    if let Some(index) = number.checked_sub(1).filter(|index| *index < 4) {
        let introducer = match parameter(modifiers) == 1 {
            true => SS3,
            false => CSI,
        };
        return Some(sequence(introducer, b'P' + index, modifiers));
    }
    const NUMBERED: [u8; 8] = [15, 17, 18, 19, 20, 21, 23, 24];
    let index = usize::from(number.checked_sub(5)?);
    Some(tilde(*NUMBERED.get(index)?, modifiers))
}

fn sequence(introducer: &[u8], final_byte: u8, modifiers: Modifiers) -> Vec<u8> {
    let mut bytes = introducer.to_vec();
    if parameter(modifiers) != 1 {
        bytes.extend_from_slice(b"1;");
        bytes.extend_from_slice(parameter(modifiers).to_string().as_bytes());
    }
    bytes.push(final_byte);
    bytes
}

fn parameter(modifiers: Modifiers) -> u8 {
    1 + u8::from(modifiers.shift) + 2 * u8::from(modifiers.alt) + 4 * u8::from(modifiers.control)
}

fn escaped(bytes: Vec<u8>, modifiers: Modifiers) -> Vec<u8> {
    if !modifiers.alt {
        return bytes;
    }
    let mut escaped = Vec::with_capacity(bytes.len() + 1);
    escaped.push(ESC);
    escaped.extend_from_slice(&bytes);
    escaped
}
