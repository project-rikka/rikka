use crate::access_point::ApId;
use crate::failure::Failure;
use crate::ip::IpFacts;
use crate::metered::Metered;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(pub(crate) String);

impl DeviceId {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeviceKind {
    #[default]
    Other,
    Ethernet,
    Wireless,
    Modem,
    Bluetooth,
    WireGuard,
}

impl DeviceKind {
    pub(crate) fn from_code(code: u32) -> Self {
        match code {
            1 => Self::Ethernet,
            2 => Self::Wireless,
            5 => Self::Bluetooth,
            8 => Self::Modem,
            29 => Self::WireGuard,
            _ => Self::Other,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceState {
    #[default]
    Unknown,
    Unmanaged,
    Unavailable,
    Disconnected,
    Connecting,
    NeedsSecrets,
    Connected,
    Disconnecting,
    Failed,
}

impl DeviceState {
    pub(crate) fn from_code(code: u32) -> Self {
        match code {
            10 => Self::Unmanaged,
            20 => Self::Unavailable,
            30 => Self::Disconnected,
            40 | 50 | 70 | 80 | 90 => Self::Connecting,
            60 => Self::NeedsSecrets,
            100 => Self::Connected,
            110 => Self::Disconnecting,
            120 => Self::Failed,
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub fn is_busy(self) -> bool {
        matches!(self, Self::Connecting | Self::Disconnecting)
    }

    #[must_use]
    pub fn is_ready(self) -> bool {
        !matches!(self, Self::Unknown | Self::Unmanaged | Self::Unavailable)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wired {
    pub carrier: bool,
    pub speed: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wireless {
    pub bitrate: u32,
    pub access_point: Option<ApId>,
    pub last_scan: i64,
    pub hotspot_capable: bool,
    pub permanent_address: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: DeviceId,
    pub kind: DeviceKind,
    pub interface: String,
    pub state: DeviceState,
    pub failure: Failure,
    pub autoconnect: bool,
    pub metered: Metered,
    pub mtu: u32,
    pub driver: String,
    pub hardware_address: String,
    pub active: Option<String>,
    pub ip4: IpFacts,
    pub ip6: IpFacts,
    pub wired: Option<Wired>,
    pub wireless: Option<Wireless>,
}

impl Device {
    #[must_use]
    pub fn title(&self) -> &str {
        &self.interface
    }
}
