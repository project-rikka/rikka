use crate::reading::{Charge, Warning};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(pub(crate) String);

impl DeviceId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    Mouse,
    Keyboard,
    Touchpad,
    Pen,
    Tablet,
    Phone,
    Headset,
    Headphones,
    Speakers,
    GamingInput,
    Wearable,
    RemoteControl,
    MediaPlayer,
    Computer,
    Ups,
    Other,
}

impl DeviceKind {
    pub(crate) fn from_code(code: u32) -> Option<DeviceKind> {
        Some(match code {
            0..=2 => return None,
            3 => DeviceKind::Ups,
            5 => DeviceKind::Mouse,
            6 => DeviceKind::Keyboard,
            8 => DeviceKind::Phone,
            9 => DeviceKind::MediaPlayer,
            10 => DeviceKind::Tablet,
            11 => DeviceKind::Computer,
            12 => DeviceKind::GamingInput,
            13 => DeviceKind::Pen,
            14 => DeviceKind::Touchpad,
            17 => DeviceKind::Headset,
            18 => DeviceKind::Speakers,
            19 => DeviceKind::Headphones,
            22 => DeviceKind::RemoteControl,
            26 => DeviceKind::Wearable,
            _ => DeviceKind::Other,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    #[default]
    Unknown,
    Critical,
    Low,
    Normal,
    High,
    Full,
}

impl Level {
    pub(crate) fn from_code(code: u32) -> Option<Level> {
        match code {
            3 => Some(Level::Low),
            4 => Some(Level::Critical),
            6 => Some(Level::Normal),
            7 => Some(Level::High),
            8 => Some(Level::Full),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: DeviceId,
    pub kind: DeviceKind,
    pub name: String,
    pub charge: Option<f32>,
    pub level: Level,
    pub state: Charge,
    pub warning: Warning,
    pub icon: Option<String>,
}
