use crate::device::DeviceId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Question {
    Confirm(u32),
    Display { passkey: u32, entered: u16 },
    DisplayPin(String),
    AskPasskey,
    AskPin,
    Authorize,
    AuthorizeService(String),
}

impl Question {
    #[must_use]
    pub fn is_asking(&self) -> bool {
        matches!(self, Self::AskPasskey | Self::AskPin)
    }

    #[must_use]
    pub fn is_answerable(&self) -> bool {
        !matches!(self, Self::Display { .. } | Self::DisplayPin(_))
    }
}

#[derive(Clone, Debug)]
pub struct Prompt {
    pub id: u64,
    pub device: DeviceId,
    pub question: Question,
}

#[derive(Clone, Debug)]
pub enum Reply {
    Yes,
    No,
    Passkey(u32),
    Pin(String),
}
