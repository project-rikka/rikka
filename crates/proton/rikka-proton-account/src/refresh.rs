use serde::{Deserialize, Serialize};

const MAIL: i32 = 1;
const CONTACTS: i32 = 2;
const EVERYTHING: i32 = 255;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Refresh(i32);

impl Refresh {
    #[must_use]
    pub fn from_bits(bits: i32) -> Refresh {
        Refresh(bits)
    }

    #[must_use]
    pub fn bits(self) -> i32 {
        self.0
    }

    #[must_use]
    pub fn is_none(self) -> bool {
        self.0 == 0
    }

    #[must_use]
    pub fn includes_everything(self) -> bool {
        self.0 & EVERYTHING == EVERYTHING
    }

    #[must_use]
    pub fn includes_mail(self) -> bool {
        self.0 & MAIL != 0
    }

    #[must_use]
    pub fn includes_contacts(self) -> bool {
        self.0 & CONTACTS != 0
    }
}
