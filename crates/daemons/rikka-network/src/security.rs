#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Security {
    #[default]
    Open,
    Enhanced,
    Wep,
    Personal,
    Wpa3Personal,
    Enterprise,
    Wpa3Enterprise,
}

const PRIVACY: u32 = 0x1;
const WPS: u32 = 0x2;
const KEY_MGMT_PSK: u32 = 0x100;
const KEY_MGMT_802_1X: u32 = 0x200;
const KEY_MGMT_SAE: u32 = 0x400;
const KEY_MGMT_OWE: u32 = 0x800;
const KEY_MGMT_OWE_TM: u32 = 0x1000;
const KEY_MGMT_EAP_SUITE_B_192: u32 = 0x2000;

impl Security {
    pub(crate) fn read(flags: u32, wpa: u32, rsn: u32) -> Self {
        let both = wpa | rsn;
        if rsn & KEY_MGMT_EAP_SUITE_B_192 != 0 {
            return Self::Wpa3Enterprise;
        }
        if both & KEY_MGMT_802_1X != 0 {
            return Self::Enterprise;
        }
        if rsn & KEY_MGMT_SAE != 0 {
            return Self::Wpa3Personal;
        }
        if both & KEY_MGMT_PSK != 0 {
            return Self::Personal;
        }
        if both & (KEY_MGMT_OWE | KEY_MGMT_OWE_TM) != 0 {
            return Self::Enhanced;
        }
        if flags & PRIVACY != 0 {
            return Self::Wep;
        }
        Self::Open
    }

    pub(crate) fn supports_wps(flags: u32) -> bool {
        flags & WPS != 0
    }

    #[must_use]
    pub fn needs_passphrase(self) -> bool {
        matches!(self, Self::Wep | Self::Personal | Self::Wpa3Personal)
    }

    #[must_use]
    pub fn needs_account(self) -> bool {
        matches!(self, Self::Enterprise | Self::Wpa3Enterprise)
    }

    #[must_use]
    pub fn is_open(self) -> bool {
        matches!(self, Self::Open | Self::Enhanced)
    }

    pub(crate) fn key_management(self) -> Option<&'static str> {
        match self {
            Self::Open => None,
            Self::Enhanced => Some("owe"),
            Self::Wep => Some("none"),
            Self::Personal => Some("wpa-psk"),
            Self::Wpa3Personal => Some("sae"),
            Self::Enterprise => Some("wpa-eap"),
            Self::Wpa3Enterprise => Some("wpa-eap-suite-b-192"),
        }
    }

    pub(crate) fn from_key_management(key_management: &str, eap: bool) -> Self {
        match key_management {
            "none" => Self::Wep,
            "owe" => Self::Enhanced,
            "sae" => Self::Wpa3Personal,
            "wpa-eap-suite-b-192" => Self::Wpa3Enterprise,
            "wpa-eap" | "ieee8021x" => Self::Enterprise,
            "wpa-psk" | "wpa-none" => Self::Personal,
            _ if eap => Self::Enterprise,
            _ => Self::Open,
        }
    }
}
