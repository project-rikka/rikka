use crate::config::protocol::Transmission;
use crate::config::wireguard::WireGuardConfig;

#[must_use]
#[derive(Debug, Clone)]
pub struct ProtunConfig {
    peer: String,
    server_ip: String,
    transmission: Transmission,
    ports: Vec<u16>,
    wireguard: WireGuardConfig,
}

impl ProtunConfig {
    pub fn new(
        peer: impl Into<String>,
        server_ip: impl Into<String>,
        transmission: Transmission,
        wireguard: WireGuardConfig,
    ) -> ProtunConfig {
        ProtunConfig {
            peer: peer.into(),
            server_ip: server_ip.into(),
            transmission,
            ports: Vec::new(),
            wireguard,
        }
    }

    pub fn ports(mut self, ports: impl IntoIterator<Item = u16>) -> ProtunConfig {
        self.ports = ports.into_iter().collect();
        self
    }

    #[must_use]
    pub fn peer(&self) -> &str {
        &self.peer
    }

    #[must_use]
    pub fn server_ip(&self) -> &str {
        &self.server_ip
    }

    #[must_use]
    pub fn transmission(&self) -> Transmission {
        self.transmission
    }

    #[must_use]
    pub fn transport_ports(&self) -> &[u16] {
        &self.ports
    }

    #[must_use]
    pub fn wireguard(&self) -> &WireGuardConfig {
        &self.wireguard
    }
}
