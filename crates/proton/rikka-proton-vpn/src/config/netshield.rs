#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NetShieldLevel {
    #[default]
    Off,
    Malware,
    Advertisements,
    AdultContent,
}

impl NetShieldLevel {
    #[must_use]
    pub fn level(self) -> u8 {
        match self {
            NetShieldLevel::Off => 0,
            NetShieldLevel::Malware => 1,
            NetShieldLevel::Advertisements => 2,
            NetShieldLevel::AdultContent => 3,
        }
    }

    #[must_use]
    pub fn from_level(level: u8) -> Option<NetShieldLevel> {
        match level {
            0 => Some(NetShieldLevel::Off),
            1 => Some(NetShieldLevel::Malware),
            2 => Some(NetShieldLevel::Advertisements),
            3 => Some(NetShieldLevel::AdultContent),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_on(self) -> bool {
        self != NetShieldLevel::Off
    }
}
