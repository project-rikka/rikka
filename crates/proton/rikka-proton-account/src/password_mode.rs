use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
pub enum PasswordMode {
    #[default]
    One,
    Two,
}

impl PasswordMode {
    #[must_use]
    pub fn needs_mailbox_password(self) -> bool {
        self == PasswordMode::Two
    }
}

impl From<i32> for PasswordMode {
    fn from(value: i32) -> PasswordMode {
        match value {
            2 => PasswordMode::Two,
            _ => PasswordMode::One,
        }
    }
}

impl From<PasswordMode> for i32 {
    fn from(mode: PasswordMode) -> i32 {
        match mode {
            PasswordMode::One => 1,
            PasswordMode::Two => 2,
        }
    }
}
