pub(crate) mod dbus;

mod eap;
mod ip;
mod mac;
mod proxy;
mod wireless;

pub use eap::{Eap, EapMethod, Phase2};
pub use ip::{IpMethod, IpProfile};
pub use mac::MacAddress;
pub use proxy::{ProxyMethod, ProxyProfile};
pub use wireless::{SecurityProfile, WirelessProfile};

pub(crate) use dbus::{merge, read};

use crate::metered::Metered;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProfileKind {
    #[default]
    Other,
    Ethernet,
    Wireless,
    WireGuard,
    Vpn(String),
}

impl ProfileKind {
    #[must_use]
    pub fn is_vpn(&self) -> bool {
        matches!(self, Self::Vpn(_) | Self::WireGuard)
    }

    #[must_use]
    pub fn is_physical(&self) -> bool {
        self.hardware_setting().is_some()
    }

    pub(crate) fn read(kind: &str, service: &str) -> Self {
        match kind {
            "802-3-ethernet" => Self::Ethernet,
            "802-11-wireless" => Self::Wireless,
            "wireguard" => Self::WireGuard,
            "vpn" => Self::Vpn(service.to_owned()),
            _ => Self::Other,
        }
    }

    pub(crate) fn hardware_setting(&self) -> Option<&'static str> {
        match self {
            Self::Ethernet => Some("802-3-ethernet"),
            Self::Wireless => Some("802-11-wireless"),
            Self::WireGuard | Self::Vpn(_) | Self::Other => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub uuid: String,
    pub id: String,
    pub kind: ProfileKind,
    pub interface: String,
    pub autoconnect: bool,
    pub autoconnect_priority: i32,
    pub metered: Metered,
    pub shared: bool,
    pub mtu: u32,
    pub address: MacAddress,
    pub wireless: Option<WirelessProfile>,
    pub security: Option<SecurityProfile>,
    pub eap: Option<Eap>,
    pub ipv4: IpProfile,
    pub ipv6: IpProfile,
    pub proxy: ProxyProfile,
    pub timestamp: u64,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            uuid: String::new(),
            id: String::new(),
            kind: ProfileKind::default(),
            interface: String::new(),
            autoconnect: true,
            autoconnect_priority: 0,
            metered: Metered::default(),
            shared: false,
            mtu: 0,
            address: MacAddress::default(),
            wireless: None,
            security: None,
            eap: None,
            ipv4: IpProfile::default(),
            ipv6: IpProfile::default(),
            proxy: ProxyProfile::default(),
            timestamp: 0,
        }
    }
}

impl Profile {
    #[must_use]
    pub fn ssid(&self) -> Option<&crate::ssid::Ssid> {
        self.wireless.as_ref().map(|wireless| &wireless.ssid)
    }

    #[must_use]
    pub fn has_secret(&self) -> bool {
        self.security
            .as_ref()
            .is_some_and(|security| !security.security.is_open())
            || self.eap.is_some()
    }
}
