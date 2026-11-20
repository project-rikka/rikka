use rikka_input::evdev;
use rikka_types::Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cap {
    Named(Key),
    Levels(char, char),
}

impl Cap {
    #[must_use]
    pub fn key(self) -> Key {
        match self {
            Cap::Named(key) => key,
            Cap::Levels(plain, _) => Key::Char(plain),
        }
    }

    #[must_use]
    pub fn code(self) -> Option<u32> {
        evdev::key_code(self.key())
    }
}
