use std::ops::{BitOr, BitOrAssign};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Char(char),
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Insert,
    Space,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
    PrintScreen,
    Menu,
    BrowserBack,
    BrowserForward,
    BrowserRefresh,
    BrowserHome,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Find,
    F(u8),
    Convert,
    NonConvert,
    ZenkakuHankaku,
    KanaMode,
    Eisu,
    HangulMode,
    HanjaMode,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
    MetaLeft,
    MetaRight,
    Unidentified,
}

impl Key {
    #[must_use]
    pub fn to_lowercase(self) -> Self {
        let Self::Char(character) = self else {
            return self;
        };
        let mut folded = character.to_lowercase();
        match (folded.next(), folded.next()) {
            (Some(lowered), None) => Self::Char(lowered),
            _ => self,
        }
    }

    #[must_use]
    pub fn is_modifier(self) -> bool {
        matches!(
            self,
            Self::ShiftLeft
                | Self::ShiftRight
                | Self::ControlLeft
                | Self::ControlRight
                | Self::AltLeft
                | Self::AltRight
                | Self::MetaLeft
                | Self::MetaRight
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub logo: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        shift: false,
        control: false,
        alt: false,
        logo: false,
    };
    pub const SHIFT: Self = Self {
        shift: true,
        control: false,
        alt: false,
        logo: false,
    };
    pub const CONTROL: Self = Self {
        shift: false,
        control: true,
        alt: false,
        logo: false,
    };
    pub const ALT: Self = Self {
        shift: false,
        control: false,
        alt: true,
        logo: false,
    };
    pub const LOGO: Self = Self {
        shift: false,
        control: false,
        alt: false,
        logo: true,
    };
    pub const CONTROL_SHIFT: Self = Self {
        shift: true,
        control: true,
        alt: false,
        logo: false,
    };
}

impl BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self {
            shift: self.shift | rhs.shift,
            control: self.control | rhs.control,
            alt: self.alt | rhs.alt,
            logo: self.logo | rhs.logo,
        }
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyLocation {
    #[default]
    Standard,
    Keypad,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyCode(u32);

impl KeyCode {
    pub const UNKNOWN: KeyCode = KeyCode(0);

    const XKB_OFFSET: u32 = 8;

    #[must_use]
    pub const fn from_evdev(code: u32) -> KeyCode {
        KeyCode(code)
    }

    #[must_use]
    pub const fn evdev(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn xkb(self) -> u32 {
        match self.0 {
            0 => 0,
            code => code + Self::XKB_OFFSET,
        }
    }

    #[must_use]
    pub const fn is_known(self) -> bool {
        self.0 != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Back,
    Forward,
    Other(u16),
}

pub const SCROLL_LINE: f32 = 40.0;
