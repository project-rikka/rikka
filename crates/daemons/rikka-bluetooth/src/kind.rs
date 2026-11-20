#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeviceKind {
    #[default]
    Unknown,
    Computer,
    Phone,
    Headset,
    Headphones,
    Speaker,
    Keyboard,
    Mouse,
    Tablet,
    Gamepad,
    Watch,
    Printer,
    Camera,
    Display,
    Network,
    Health,
}

impl DeviceKind {
    pub(crate) fn from_icon(icon: &str) -> Option<Self> {
        Some(match icon {
            "computer" => Self::Computer,
            "phone" => Self::Phone,
            "audio-headset" => Self::Headset,
            "audio-headphones" => Self::Headphones,
            "audio-card" | "audio-speakers" => Self::Speaker,
            "input-keyboard" => Self::Keyboard,
            "input-mouse" => Self::Mouse,
            "input-tablet" => Self::Tablet,
            "input-gaming" => Self::Gamepad,
            "printer" => Self::Printer,
            "camera-photo" | "camera-video" | "scanner" => Self::Camera,
            "video-display" | "tv" => Self::Display,
            "network-wireless" | "modem" => Self::Network,
            _ => return None,
        })
    }

    pub(crate) fn from_class(class: u32) -> Self {
        let major = (class >> 8) & 0x1f;
        let minor = (class >> 2) & 0x3f;
        match (major, minor) {
            (1, _) => Self::Computer,
            (2, _) => Self::Phone,
            (3, _) => Self::Network,
            (4, 1 | 2) => Self::Headset,
            (4, 6) => Self::Headphones,
            (4, 5 | 7 | 8 | 10) => Self::Speaker,
            (4, 9 | 11 | 12 | 13) => Self::Display,
            (5, minor) if minor & 0x10 != 0 => Self::Keyboard,
            (5, minor) if minor & 0x20 != 0 => Self::Mouse,
            (5, 2) => Self::Gamepad,
            (6, _) => Self::Printer,
            (7, _) => Self::Watch,
            (9, _) => Self::Health,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn from_appearance(appearance: u16) -> Self {
        match (appearance >> 6, appearance & 0x3f) {
            (1, _) => Self::Phone,
            (2, _) => Self::Computer,
            (3, _) => Self::Watch,
            (5, _) => Self::Display,
            (10, _) => Self::Speaker,
            (12..=14 | 16..=18, _) => Self::Health,
            (15, 1) => Self::Keyboard,
            (15, 2) => Self::Mouse,
            (15, 3 | 4) => Self::Gamepad,
            (15, 5) => Self::Tablet,
            _ => Self::Unknown,
        }
    }
}
