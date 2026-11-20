use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum VpnProtocol {
    #[serde(rename = "WireGuard")]
    WireGuard,
    #[serde(rename = "OpenVPN")]
    OpenVpn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Transmission {
    #[serde(rename = "UDP")]
    Udp,
    #[serde(rename = "TCP")]
    Tcp,
    #[serde(rename = "TLS")]
    Tls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Protocol {
    #[serde(rename = "WireGuardUDP")]
    WireGuardUdp,
    #[serde(rename = "WireGuardTCP")]
    WireGuardTcp,
    #[serde(rename = "WireGuardTLS")]
    WireGuardTls,
    #[serde(rename = "OpenVPNUDP")]
    OpenVpnUdp,
    #[serde(rename = "OpenVPNTCP")]
    OpenVpnTcp,
}

impl VpnProtocol {
    #[must_use]
    pub fn api_name(self) -> &'static str {
        match self {
            VpnProtocol::WireGuard => "WireGuard",
            VpnProtocol::OpenVpn => "OpenVPN",
        }
    }
}

impl Transmission {
    #[must_use]
    pub fn api_name(self) -> &'static str {
        match self {
            Transmission::Udp => "UDP",
            Transmission::Tcp => "TCP",
            Transmission::Tls => "TLS",
        }
    }
}

impl Protocol {
    pub const ALL: [Protocol; 5] = [
        Protocol::WireGuardUdp,
        Protocol::WireGuardTcp,
        Protocol::WireGuardTls,
        Protocol::OpenVpnUdp,
        Protocol::OpenVpnTcp,
    ];

    #[must_use]
    pub fn api_name(self) -> &'static str {
        match self {
            Protocol::WireGuardUdp => "WireGuardUDP",
            Protocol::WireGuardTcp => "WireGuardTCP",
            Protocol::WireGuardTls => "WireGuardTLS",
            Protocol::OpenVpnUdp => "OpenVPNUDP",
            Protocol::OpenVpnTcp => "OpenVPNTCP",
        }
    }

    #[must_use]
    pub fn vpn(self) -> VpnProtocol {
        match self {
            Protocol::WireGuardUdp | Protocol::WireGuardTcp | Protocol::WireGuardTls => {
                VpnProtocol::WireGuard
            }
            Protocol::OpenVpnUdp | Protocol::OpenVpnTcp => VpnProtocol::OpenVpn,
        }
    }

    #[must_use]
    pub fn transmission(self) -> Transmission {
        match self {
            Protocol::WireGuardUdp | Protocol::OpenVpnUdp => Transmission::Udp,
            Protocol::WireGuardTcp | Protocol::OpenVpnTcp => Transmission::Tcp,
            Protocol::WireGuardTls => Transmission::Tls,
        }
    }

    #[must_use]
    pub fn from_parts(vpn: VpnProtocol, transmission: Transmission) -> Option<Protocol> {
        Protocol::ALL
            .into_iter()
            .find(|protocol| protocol.vpn() == vpn && protocol.transmission() == transmission)
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.api_name())
    }
}

impl FromStr for Protocol {
    type Err = Error;

    fn from_str(name: &str) -> Result<Protocol, Error> {
        Protocol::ALL
            .into_iter()
            .find(|protocol| protocol.api_name() == name)
            .ok_or_else(|| Error::UnknownProtocol {
                name: name.to_owned(),
            })
    }
}
