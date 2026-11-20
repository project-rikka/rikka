use futures_util::StreamExt;
use rikka_network::{
    ActiveId, ActiveState, OpenVpnTransport, OpenVpnTunnel, ProtunPeer, ProtunTunnel, SavedTunnel,
    Tunnel, TunnelKind, TunnelState, Tunnels, WireGuardPeer, WireGuardTunnel,
};

use crate::config::{
    OpenVpnConfig, Protocol, ProtunConfig, Transmission, VpnProtocol, WireGuardConfig,
};
use crate::connect::tunnel::{TunnelControl, TunnelEvent, TunnelEvents, TunnelPlan};
use crate::error::Result;

const INTERFACE: &str = "proton0";

#[derive(Debug, Clone)]
pub struct NetworkManager {
    tunnels: Tunnels,
    openvpn: bool,
    protun: bool,
}

#[derive(Debug)]
pub struct NetworkManagerTunnel {
    saved: SavedTunnel,
    active: ActiveId,
}

impl NetworkManager {
    pub async fn system() -> Result<NetworkManager> {
        Ok(NetworkManager {
            tunnels: Tunnels::system().await?,
            openvpn: TunnelKind::OpenVpn.is_installed(),
            protun: TunnelKind::Protun.is_installed(),
        })
    }

    async fn activate(&self, saved: &SavedTunnel) -> Result<(ActiveId, TunnelEvents)> {
        let active = self.tunnels.activate(saved).await?;
        match self.tunnels.states(&active).await {
            Ok(states) => Ok((active, states.map(tunnel_event).boxed())),
            Err(error) => {
                if let Err(error) = self.tunnels.deactivate(&active).await {
                    tracing::warn!(%error, "failed to deactivate NetworkManager connection");
                }
                Err(error.into())
            }
        }
    }
}

impl TunnelControl for NetworkManager {
    type Tunnel = NetworkManagerTunnel;

    fn supports(&self, protocol: Protocol) -> bool {
        match (protocol.vpn(), protocol.transmission()) {
            (VpnProtocol::WireGuard, Transmission::Udp) => true,
            (VpnProtocol::WireGuard, _) => self.protun,
            (VpnProtocol::OpenVpn, _) => self.openvpn,
        }
    }

    async fn open(
        &self,
        name: &str,
        plan: &TunnelPlan,
    ) -> Result<(NetworkManagerTunnel, TunnelEvents)> {
        let saved = self.tunnels.add(tunnel(name, plan)).await?;
        match self.activate(&saved).await {
            Ok((active, events)) => Ok((NetworkManagerTunnel { saved, active }, events)),
            Err(error) => {
                if let Err(remove_error) = self.tunnels.remove(&saved).await {
                    tracing::warn!(error = %remove_error, "failed to remove inactive NetworkManager connection");
                }
                Err(error)
            }
        }
    }

    async fn renew(
        &self,
        _tunnel: &NetworkManagerTunnel,
        name: &str,
        plan: &TunnelPlan,
    ) -> Result<()> {
        match plan {
            TunnelPlan::OpenVpn(config) => {
                Ok(self.tunnels.rewrite_files(&openvpn(name, config))?)
            }
            TunnelPlan::WireGuard(_) | TunnelPlan::Protun(_) => Ok(()),
        }
    }

    async fn close(&self, tunnel: NetworkManagerTunnel) -> Result<()> {
        let deactivated = self.tunnels.deactivate(&tunnel.active).await;
        self.tunnels.remove(&tunnel.saved).await?;
        Ok(deactivated?)
    }
}

fn tunnel(name: &str, plan: &TunnelPlan) -> Tunnel {
    match plan {
        TunnelPlan::WireGuard(config) => wireguard(name, config).into(),
        TunnelPlan::OpenVpn(config) => openvpn(name, config).into(),
        TunnelPlan::Protun(config) => protun(name, config).into(),
    }
}

fn wireguard(name: &str, config: &WireGuardConfig) -> WireGuardTunnel {
    let peer = WireGuardPeer::new(config.public_key().to_base64())
        .endpoint(config.endpoint())
        .allowed_ips(config.routes())
        .keepalive(u32::from(config.persistent_keepalive()));
    WireGuardTunnel::new(name, config.private_key().to_base64())
        .interface(INTERFACE)
        .addresses(config.addresses())
        .dns(config.dns_servers())
        .exclusive_dns(true)
        .peer(peer)
}

fn openvpn(name: &str, config: &OpenVpnConfig) -> OpenVpnTunnel {
    let transport = match config.uses_tcp() {
        true => OpenVpnTransport::Tcp,
        false => OpenVpnTransport::Udp,
    };
    config.remote_ports().iter().fold(
        OpenVpnTunnel::new(name)
            .transport(transport)
            .ca(config.ca())
            .tls_crypt(config.tls_crypt())
            .certificate(config.certificate())
            .encrypted_key(config.encrypted_key(), config.key_passphrase().expose())
            .verify_name(config.domain())
            .ipv6(config.uses_ipv6()),
        |tunnel, port| tunnel.remote(config.server_ip(), *port),
    )
}

fn protun(name: &str, config: &ProtunConfig) -> ProtunTunnel {
    let wireguard = config.wireguard();
    let ports = config.transport_ports().iter().copied();
    let peer = ProtunPeer::new(
        config.peer(),
        config.server_ip(),
        wireguard.public_key().to_base64(),
    );
    let peer = match config.transmission() {
        Transmission::Udp => peer.udp_ports(ports),
        Transmission::Tcp => peer.tcp_ports(ports),
        Transmission::Tls => peer.tls_ports(ports),
    };
    ProtunTunnel::new(name, wireguard.private_key().to_base64())
        .interface(INTERFACE)
        .addresses(wireguard.addresses())
        .dns(wireguard.dns_servers())
        .peer(peer)
}

fn tunnel_event(state: TunnelState) -> TunnelEvent {
    match state.state {
        ActiveState::Activated => TunnelEvent::Up,
        ActiveState::Deactivating | ActiveState::Deactivated => TunnelEvent::Down {
            deliberate: state.reason.is_deliberate(),
        },
        ActiveState::Unknown | ActiveState::Activating => TunnelEvent::Connecting,
    }
}
