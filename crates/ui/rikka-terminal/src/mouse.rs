use alacritty_terminal::term::TermMode;
use rikka_types::{Modifiers, PointerButton};

const FURTHEST: usize = 222;

const FURTHEST_UTF8: usize = 0x7ff - OFFSET;

const OFFSET: usize = 33;

const RELEASED: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Button {
    Left,
    Middle,
    Right,
    Back,
    Forward,
    WheelUp,
    WheelDown,
    WheelLeft,
    WheelRight,
}

impl Button {
    #[must_use]
    pub(crate) fn from_pointer_button(button: PointerButton) -> Option<Self> {
        Some(match button {
            PointerButton::Primary => Self::Left,
            PointerButton::Middle => Self::Middle,
            PointerButton::Secondary => Self::Right,
            PointerButton::Back => Self::Back,
            PointerButton::Forward => Self::Forward,
            PointerButton::Other(_) => return None,
        })
    }

    fn code(self) -> u8 {
        match self {
            Self::Left => 0,
            Self::Middle => 1,
            Self::Right => 2,
            Self::WheelUp => 64,
            Self::WheelDown => 65,
            Self::WheelLeft => 66,
            Self::WheelRight => 67,
            Self::Back => 128,
            Self::Forward => 129,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Motion {
    Pressed(Button),
    Released(Button),
    Moved(Option<Button>),
}

impl Motion {
    fn button(self) -> Option<Button> {
        match self {
            Self::Pressed(button) | Self::Released(button) => Some(button),
            Self::Moved(button_code) => button_code,
        }
    }

    fn is_moved(self) -> bool {
        matches!(self, Self::Moved(_))
    }

    fn is_released(self) -> bool {
        matches!(self, Self::Released(_))
    }

    fn is_reported(self, mode: TermMode) -> bool {
        match self {
            Self::Pressed(_) | Self::Released(_) => mode.intersects(TermMode::MOUSE_MODE),
            Self::Moved(Some(_)) => mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION),
            Self::Moved(None) => mode.contains(TermMode::MOUSE_MOTION),
        }
    }
}

#[must_use]
pub(crate) fn report(
    motion: Motion,
    column: usize,
    line: usize,
    modifiers: Modifiers,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if !motion.is_reported(mode) {
        return None;
    }
    let coded = |button: u8| {
        usize::from(button)
            + usize::from(modifier_bits(modifiers))
            + 32 * usize::from(motion.is_moved())
    };
    let button_code = motion.button().map_or(RELEASED, Button::code);

    if mode.contains(TermMode::SGR_MOUSE) {
        let ending = match motion.is_released() {
            true => 'm',
            false => 'M',
        };
        let code = coded(button_code);
        return Some(format!("\x1b[<{code};{};{}{ending}", column + 1, line + 1).into_bytes());
    }

    let code = match motion.is_released() {
        true => coded(RELEASED),
        false => coded(button_code),
    };
    let mut bytes = b"\x1b[M".to_vec();
    if mode.contains(TermMode::UTF8_MOUSE) {
        if column > FURTHEST_UTF8 || line > FURTHEST_UTF8 {
            return None;
        }
        push_utf8(&mut bytes, code + 32);
        push_utf8(&mut bytes, column + OFFSET);
        push_utf8(&mut bytes, line + OFFSET);
        return Some(bytes);
    }
    if column > FURTHEST || line > FURTHEST {
        return None;
    }
    bytes.push(u8::try_from(code + 32).ok()?);
    bytes.push((column + OFFSET) as u8);
    bytes.push((line + OFFSET) as u8);
    Some(bytes)
}

fn push_utf8(bytes: &mut Vec<u8>, value: usize) {
    let mut room = [0; 4];
    let Some(character) = u32::try_from(value).ok().and_then(char::from_u32) else {
        return;
    };
    bytes.extend_from_slice(character.encode_utf8(&mut room).as_bytes());
}

fn modifier_bits(modifiers: Modifiers) -> u8 {
    4 * u8::from(modifiers.shift) + 8 * u8::from(modifiers.alt) + 16 * u8::from(modifiers.control)
}
