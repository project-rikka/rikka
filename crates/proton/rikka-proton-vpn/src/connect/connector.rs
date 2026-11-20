use std::collections::VecDeque;
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::client::VpnClient;
use crate::connect::credentials::{Credentials, Keyring};
use crate::connect::settings::ConnectionSettings;
use crate::connect::state::ConnectionState;
use crate::connect::supervisor::{AgentSetup, Controls, Supervisor, select_server};
use crate::connect::tunnel::TunnelControl;
use crate::connect::unreachable::UnreachableTracker;
use crate::error::{Error, Result};
use crate::servers::{LogicalServer, ServerQuery};

pub struct Connector<T: TunnelControl> {
    client: VpnClient,
    tunnels: Arc<T>,
    keyring: Keyring,
    settings: ConnectionSettings,
    agent: AgentSetup,
}

pub struct VpnConnection {
    controls: watch::Sender<Controls>,
    state: watch::Receiver<ConnectionState>,
    task: Option<JoinHandle<()>>,
}

impl<T: TunnelControl> Connector<T> {
    #[must_use]
    pub fn new(client: VpnClient, tunnels: T) -> Connector<T> {
        Connector {
            client,
            tunnels: Arc::new(tunnels),
            keyring: Keyring::new(Credentials::generate()),
            settings: ConnectionSettings::default(),
            agent: AgentSetup::default(),
        }
    }

    #[must_use]
    pub fn credentials(self, credentials: Credentials) -> Connector<T> {
        Connector {
            keyring: Keyring::new(credentials),
            ..self
        }
    }

    #[must_use]
    pub fn settings(self, settings: ConnectionSettings) -> Connector<T> {
        Connector { settings, ..self }
    }

    #[must_use]
    pub fn agent_address(mut self, address: SocketAddr) -> Connector<T> {
        self.agent.address = Some(address);
        self
    }

    #[must_use]
    pub fn agent_roots(mut self, pem: impl Into<String>) -> Connector<T> {
        self.agent.roots = Some(pem.into());
        self
    }

    #[must_use]
    pub fn current_credentials(&self) -> Credentials {
        self.keyring.current_credentials()
    }

    pub async fn connect(
        &self,
        servers: &[LogicalServer],
        query: &ServerQuery,
    ) -> Result<VpnConnection> {
        let protocol = query.selected_protocol();
        if !self.tunnels.supports(protocol) {
            return Err(Error::UnsupportedProtocol { protocol });
        }

        let (account, config) =
            futures_util::try_join!(self.client.vpn_info(), self.client.client_config())?;
        let tier = account.vpn.tier();
        let best = query.best(servers, tier).ok_or(Error::NoServer)?;
        if !tier.includes(best.tier) {
            return Err(Error::InsufficientTier {
                server: best.name.clone(),
                tier: best.tier,
            });
        }

        let mut candidates: VecDeque<LogicalServer> = VecDeque::new();
        if best.is_online() {
            candidates.push_back(best.clone());
        }
        candidates.extend(query.fallbacks(servers, tier, best).into_iter().cloned());
        let first = candidates.front().ok_or(Error::NoServer)?;
        let server = select_server(first, protocol, &config.default_ports)?.choice;

        let (controls, control_receiver) = watch::channel(Controls {
            settings: self.settings.clone(),
            connectivity: true,
            status_requests: 0,
            statistics: false,
            disconnect: false,
        });
        let (state, state_receiver) = watch::channel(ConnectionState::Connecting { server });
        let supervisor = Supervisor {
            client: self.client.clone(),
            tunnels: Arc::clone(&self.tunnels),
            keyring: self.keyring.clone(),
            settings: self.settings.clone(),
            agent: self.agent.clone(),
            ports: config.default_ports,
            protocol,
            candidates,
            controls: control_receiver,
            status_requests: 0,
            state,
            tracker: UnreachableTracker::default(),
            connectivity: true,
        };

        Ok(VpnConnection {
            controls,
            state: state_receiver,
            task: Some(tokio::spawn(supervisor.run())),
        })
    }
}

impl VpnConnection {
    #[must_use]
    pub fn state(&self) -> ConnectionState {
        self.state.borrow().clone()
    }

    #[must_use]
    pub fn watch(&self) -> watch::Receiver<ConnectionState> {
        self.state.clone()
    }

    pub async fn changed(&mut self) -> Option<ConnectionState> {
        self.state.changed().await.ok()?;
        Some(self.state.borrow_and_update().clone())
    }

    pub fn apply(&self, settings: ConnectionSettings) {
        self.controls
            .send_modify(|controls| controls.settings = settings);
    }

    pub fn request_status(&self, statistics: bool) {
        self.controls.send_modify(|controls| {
            controls.status_requests += 1;
            controls.statistics = statistics;
        });
    }

    pub fn set_connectivity(&self, available: bool) {
        self.controls
            .send_modify(|controls| controls.connectivity = available);
    }

    pub async fn disconnect(mut self) {
        self.request_disconnect();
        if let Some(task) = self.task.take()
            && let Err(error) = task.await
        {
            tracing::warn!(%error, "VPN connection failed");
        }
    }

    fn request_disconnect(&self) {
        self.controls
            .send_modify(|controls| controls.disconnect = true);
    }
}

impl Drop for VpnConnection {
    fn drop(&mut self) {
        if self.task.is_some() {
            self.request_disconnect();
        }
    }
}

impl<T: TunnelControl> Clone for Connector<T> {
    fn clone(&self) -> Connector<T> {
        Connector {
            client: self.client.clone(),
            tunnels: Arc::clone(&self.tunnels),
            keyring: self.keyring.clone(),
            settings: self.settings.clone(),
            agent: self.agent.clone(),
        }
    }
}

impl<T: TunnelControl> fmt::Debug for Connector<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connector")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for VpnConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VpnConnection")
            .field("state", &*self.state.borrow())
            .finish_non_exhaustive()
    }
}
