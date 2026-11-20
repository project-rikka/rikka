use serde::Deserialize;

use crate::config::{Protocol, Transmission, VpnProtocol};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ProtocolPorts {
    #[serde(rename = "UDP")]
    pub udp: Vec<u16>,
    #[serde(rename = "TCP")]
    pub tcp: Vec<u16>,
    #[serde(rename = "TLS")]
    pub tls: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct DefaultPorts {
    #[serde(rename = "WireGuard")]
    pub wireguard: ProtocolPorts,
    #[serde(rename = "OpenVPN")]
    pub openvpn: ProtocolPorts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct FeatureFlags {
    #[serde(rename = "ServerRefresh")]
    pub server_refresh: bool,
    #[serde(rename = "WireGuardTls")]
    pub wireguard_tls: bool,
    #[serde(rename = "PollNotificationAPI")]
    pub poll_notification_api: bool,
    #[serde(rename = "NetShield")]
    pub netshield: bool,
    #[serde(rename = "NetShieldStats")]
    pub netshield_stats: bool,
    #[serde(rename = "PortForwarding")]
    pub port_forwarding: bool,
    #[serde(rename = "ModerateNAT")]
    pub moderate_nat: bool,
    #[serde(rename = "SafeMode")]
    pub safe_mode: bool,
    #[serde(rename = "VpnAccelerator")]
    pub vpn_accelerator: bool,
    #[serde(rename = "SmartReconnect")]
    pub smart_reconnect: bool,
    #[serde(rename = "StreamingServicesLogos")]
    pub streaming_services_logos: bool,
    #[serde(rename = "Telemetry")]
    pub telemetry: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct SmartProtocol {
    #[serde(rename = "WireGuard")]
    pub wireguard: bool,
    #[serde(rename = "WireGuardTCP")]
    pub wireguard_tcp: bool,
    #[serde(rename = "WireGuardTLS")]
    pub wireguard_tls: bool,
    #[serde(rename = "OpenVPN")]
    pub openvpn: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    #[serde(rename = "DefaultPorts")]
    pub default_ports: DefaultPorts,
    #[serde(rename = "FeatureFlags")]
    pub feature_flags: FeatureFlags,
    #[serde(rename = "SmartProtocol")]
    pub smart_protocol: SmartProtocol,
    #[serde(rename = "HolesIPs")]
    pub holes_ips: Vec<String>,
    #[serde(rename = "ServerRefreshInterval")]
    pub server_refresh_interval_minutes: Option<u64>,
    #[serde(rename = "LogicalsRefreshIntervalForegroundMinutes")]
    pub logicals_refresh_foreground_minutes: Option<u64>,
    #[serde(rename = "LogicalsRefreshIntervalBackgroundMinutes")]
    pub logicals_refresh_background_minutes: Option<u64>,
    #[serde(rename = "ChangeServerAttemptLimit")]
    pub change_server_attempt_limit: Option<u32>,
    #[serde(rename = "ChangeServerShortDelayInSeconds")]
    pub change_server_short_delay_seconds: Option<u32>,
    #[serde(rename = "ChangeServerLongDelayInSeconds")]
    pub change_server_long_delay_seconds: Option<u32>,
}

impl Default for DefaultPorts {
    fn default() -> DefaultPorts {
        DefaultPorts {
            wireguard: ProtocolPorts {
                udp: vec![443, 88, 1224, 51820, 500, 4500],
                tcp: vec![443],
                tls: Vec::new(),
            },
            openvpn: ProtocolPorts {
                udp: vec![80, 51820, 4569, 1194, 5060],
                tcp: vec![443, 7770, 8443],
                tls: Vec::new(),
            },
        }
    }
}

impl Default for FeatureFlags {
    fn default() -> FeatureFlags {
        FeatureFlags {
            server_refresh: true,
            wireguard_tls: true,
            poll_notification_api: false,
            netshield: false,
            netshield_stats: false,
            port_forwarding: false,
            moderate_nat: false,
            safe_mode: false,
            vpn_accelerator: false,
            smart_reconnect: false,
            streaming_services_logos: false,
            telemetry: false,
        }
    }
}

impl Default for SmartProtocol {
    fn default() -> SmartProtocol {
        SmartProtocol {
            wireguard: true,
            wireguard_tcp: true,
            wireguard_tls: true,
            openvpn: false,
        }
    }
}

impl ProtocolPorts {
    #[must_use]
    pub fn for_transmission(&self, transmission: Transmission) -> &[u16] {
        match transmission {
            Transmission::Udp => &self.udp,
            Transmission::Tcp => &self.tcp,
            Transmission::Tls => match self.tls.is_empty() {
                true => &self.tcp,
                false => &self.tls,
            },
        }
    }
}

impl DefaultPorts {
    #[must_use]
    pub fn for_protocol(&self, protocol: Protocol) -> &[u16] {
        let ports = match protocol.vpn() {
            VpnProtocol::WireGuard => &self.wireguard,
            VpnProtocol::OpenVpn => &self.openvpn,
        };

        ports.for_transmission(protocol.transmission())
    }
}

impl SmartProtocol {
    #[must_use]
    pub fn protocols(&self) -> Vec<Protocol> {
        let candidates = [
            (self.wireguard, Protocol::WireGuardUdp),
            (self.wireguard_tcp, Protocol::WireGuardTcp),
            (self.wireguard_tls, Protocol::WireGuardTls),
            (self.openvpn, Protocol::OpenVpnUdp),
            (self.openvpn, Protocol::OpenVpnTcp),
        ];

        candidates
            .into_iter()
            .filter_map(|(enabled, protocol)| enabled.then_some(protocol))
            .collect()
    }
}
