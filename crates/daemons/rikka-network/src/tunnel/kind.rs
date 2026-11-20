use std::path::Path;

use crate::tunnel::openvpn::OpenVpnTunnel;
use crate::tunnel::protun::{self, ProtunTunnel};
use crate::tunnel::wireguard::WireGuardTunnel;

const OPENVPN_SERVICE: &str = "org.freedesktop.NetworkManager.openvpn";
const PLUGIN_FOLDERS: [&str; 3] = [
    "/etc/NetworkManager/VPN",
    "/usr/lib/NetworkManager/VPN",
    "/usr/lib64/NetworkManager/VPN",
];
const PLUGIN_EXTENSION: &str = "name";
const SERVICE_KEY: &str = "service";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tunnel {
    WireGuard(WireGuardTunnel),
    OpenVpn(OpenVpnTunnel),
    Protun(ProtunTunnel),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TunnelKind {
    WireGuard,
    OpenVpn,
    Protun,
}

impl Tunnel {
    #[must_use]
    pub fn kind(&self) -> TunnelKind {
        match self {
            Tunnel::WireGuard(_) => TunnelKind::WireGuard,
            Tunnel::OpenVpn(_) => TunnelKind::OpenVpn,
            Tunnel::Protun(_) => TunnelKind::Protun,
        }
    }
}

impl TunnelKind {
    #[must_use]
    pub fn is_installed(self) -> bool {
        match self {
            TunnelKind::WireGuard => true,
            TunnelKind::OpenVpn => plugin_installed(OPENVPN_SERVICE),
            TunnelKind::Protun => plugin_installed(protun::SERVICE),
        }
    }
}

impl From<WireGuardTunnel> for Tunnel {
    fn from(tunnel: WireGuardTunnel) -> Tunnel {
        Tunnel::WireGuard(tunnel)
    }
}

impl From<OpenVpnTunnel> for Tunnel {
    fn from(tunnel: OpenVpnTunnel) -> Tunnel {
        Tunnel::OpenVpn(tunnel)
    }
}

impl From<ProtunTunnel> for Tunnel {
    fn from(tunnel: ProtunTunnel) -> Tunnel {
        Tunnel::Protun(tunnel)
    }
}

fn plugin_installed(service: &str) -> bool {
    PLUGIN_FOLDERS
        .iter()
        .filter_map(|folder| std::fs::read_dir(folder).ok())
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == PLUGIN_EXTENSION)
        })
        .any(|path| names_service(&path, service))
}

fn names_service(path: &Path, service: &str) -> bool {
    std::fs::read_to_string(path).is_ok_and(|text| {
        text.lines().any(|line| {
            line.split_once('=')
                .is_some_and(|(key, value)| key.trim() == SERVICE_KEY && value.trim() == service)
        })
    })
}
