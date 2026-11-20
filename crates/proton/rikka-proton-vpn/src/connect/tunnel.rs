use std::future::Future;

use futures_util::stream::BoxStream;

use crate::config::{OpenVpnConfig, Protocol, ProtunConfig, WireGuardConfig};
use crate::error::Result;

pub type TunnelEvents = BoxStream<'static, TunnelEvent>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TunnelEvent {
    Connecting,
    Up,
    Down { deliberate: bool },
}

#[derive(Debug, Clone)]
pub enum TunnelPlan {
    WireGuard(WireGuardConfig),
    OpenVpn(OpenVpnConfig),
    Protun(ProtunConfig),
}

pub trait TunnelControl: Send + Sync + 'static {
    type Tunnel: Send + Sync + 'static;

    fn supports(&self, protocol: Protocol) -> bool;

    fn open(
        &self,
        name: &str,
        plan: &TunnelPlan,
    ) -> impl Future<Output = Result<(Self::Tunnel, TunnelEvents)>> + Send;

    fn renew(
        &self,
        tunnel: &Self::Tunnel,
        name: &str,
        plan: &TunnelPlan,
    ) -> impl Future<Output = Result<()>> + Send;

    fn close(&self, tunnel: Self::Tunnel) -> impl Future<Output = Result<()>> + Send;
}
