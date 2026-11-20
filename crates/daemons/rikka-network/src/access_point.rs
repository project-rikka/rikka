use crate::device::DeviceId;
use crate::security::Security;
use crate::ssid::Ssid;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ApId(pub(crate) String);

impl ApId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ApMode {
    #[default]
    Unknown,
    AdHoc,
    Infrastructure,
    Hotspot,
    Mesh,
}

impl ApMode {
    pub(crate) fn from_code(code: u32) -> Self {
        match code {
            1 => Self::AdHoc,
            2 => Self::Infrastructure,
            3 => Self::Hotspot,
            4 => Self::Mesh,
            _ => Self::Unknown,
        }
    }

    pub(crate) fn from_name(name: &str) -> Self {
        match name {
            "adhoc" => Self::AdHoc,
            "ap" => Self::Hotspot,
            "mesh" => Self::Mesh,
            _ => Self::Infrastructure,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Band {
    #[default]
    Unknown,
    Legacy,
    Fast,
    Wide,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bars {
    #[default]
    One,
    Two,
    Three,
    Four,
}

impl Bars {
    #[must_use]
    pub fn from_strength(strength: u8) -> Bars {
        match strength {
            75.. => Bars::Four,
            50..75 => Bars::Three,
            25..50 => Bars::Two,
            _ => Bars::One,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AccessPoint {
    pub id: ApId,
    pub device: DeviceId,
    pub ssid: Ssid,
    pub bssid: String,
    pub strength: u8,
    pub frequency: u32,
    pub max_bitrate: u32,
    pub security: Security,
    pub mode: ApMode,
    pub wps: bool,
    pub in_use: bool,
    pub saved: Option<String>,
}

impl AccessPoint {
    #[must_use]
    pub fn bars(&self) -> Bars {
        Bars::from_strength(self.strength)
    }
    #[must_use]
    pub fn band(&self) -> Band {
        match self.frequency {
            2_400..=2_500 => Band::Legacy,
            4_900..=5_900 => Band::Fast,
            5_925..=7_125 => Band::Wide,
            _ => Band::Unknown,
        }
    }

    #[must_use]
    pub fn channel(&self) -> u32 {
        match self.frequency {
            2_484 => 14,
            2_412..=2_472 => (self.frequency - 2_407) / 5,
            4_915..=4_980 => (self.frequency - 4_000) / 5,
            5_035..=5_895 => (self.frequency - 5_000) / 5,
            5_955..=7_115 => (self.frequency - 5_950) / 5,
            _ => 0,
        }
    }
}
