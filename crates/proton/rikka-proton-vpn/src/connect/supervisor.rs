use std::collections::VecDeque;
use std::net::SocketAddr;
use std::ops::ControlFlow;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use jiff::Timestamp;
use tokio::sync::watch;
use tokio::time::Instant;

use crate::account::DefaultPorts;
use crate::agent::{AgentConnection, AgentEvent, AgentState, AgentStatus, LocalAgent};
use crate::certificate::Certificate;
use crate::client::VpnClient;
use crate::config::{OpenVpnConfig, Protocol, Transmission, VpnProtocol};
use crate::connect::credentials::{Keyring, retry_after_failure};
use crate::connect::reaction::Reaction;
use crate::connect::settings::ConnectionSettings;
use crate::connect::state::{ConnectionState, Problem, ServerChoice};
use crate::connect::tunnel::{TunnelControl, TunnelEvent, TunnelEvents, TunnelPlan};
use crate::connect::unreachable::{UnreachableAction, UnreachableTracker};
use crate::error::{Error, Result};
use crate::key::KeyPair;
use crate::servers::{LogicalServer, ServerEntry};
use rikka_proton_pgp::Secret;

const TUNNEL_UP_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_REOPENS: u32 = 2;
const MAX_RENEWALS: u32 = 1;

#[derive(Debug, Clone)]
pub(crate) struct Controls {
    pub(crate) settings: ConnectionSettings,
    pub(crate) connectivity: bool,
    pub(crate) status_requests: u64,
    pub(crate) statistics: bool,
    pub(crate) disconnect: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AgentSetup {
    pub(crate) address: Option<SocketAddr>,
    pub(crate) roots: Option<String>,
}

pub(crate) struct Supervisor<T: TunnelControl> {
    pub(crate) client: VpnClient,
    pub(crate) tunnels: Arc<T>,
    pub(crate) keyring: Keyring,
    pub(crate) settings: ConnectionSettings,
    pub(crate) agent: AgentSetup,
    pub(crate) ports: DefaultPorts,
    pub(crate) protocol: Protocol,
    pub(crate) candidates: VecDeque<LogicalServer>,
    pub(crate) controls: watch::Receiver<Controls>,
    pub(crate) status_requests: u64,
    pub(crate) state: watch::Sender<ConnectionState>,
    pub(crate) tracker: UnreachableTracker,
    pub(crate) connectivity: bool,
}

enum Outcome {
    Disconnected,
    Failed(Problem),
    NextServer(Problem),
    Reopen(Problem),
    Renewed,
}

enum AgentStep {
    Continue,
    Exit(Outcome),
    RefreshCertificate,
}

struct ServerSession {
    name: String,
    peer: String,
    choice: ServerChoice,
    entry: ServerEntry,
    label: Option<String>,
    ports: Vec<u16>,
    passphrase: Option<Secret>,
    keys: KeyPair,
    certificate: Certificate,
    status: Option<Box<AgentStatus>>,
}

impl<T: TunnelControl> Supervisor<T> {
    pub(crate) async fn run(mut self) {
        let mut last_problem = Problem::Unreachable;
        let mut reopens = 0;
        let mut renewals = 0;

        while let Some(server) = self.candidates.front().cloned() {
            match self.attempt(&server).await {
                Outcome::Disconnected => return self.publish(ConnectionState::Disconnected),
                Outcome::Failed(problem) => {
                    return self.publish(ConnectionState::Failed { problem });
                }
                Outcome::Reopen(_) if reopens < MAX_REOPENS => reopens += 1,
                Outcome::NextServer(problem) | Outcome::Reopen(problem) => {
                    last_problem = problem;
                    self.candidates.pop_front();
                    reopens = 0;
                    renewals = 0;
                }
                Outcome::Renewed if renewals == MAX_RENEWALS => {
                    return self.publish(ConnectionState::Failed {
                        problem: Problem::Certificate(
                            "server rejected newly issued certificate".to_owned(),
                        ),
                    });
                }
                Outcome::Renewed => renewals += 1,
            }
        }

        self.publish(ConnectionState::Failed {
            problem: last_problem,
        });
    }

    async fn attempt(&mut self, server: &LogicalServer) -> Outcome {
        self.tracker.reset(false);
        let SelectedServer {
            entry,
            choice,
            ports,
        } = match select_server(server, self.protocol, &self.ports) {
            Ok(selected) => selected,
            Err(error) => return Outcome::NextServer(Problem::Tunnel(error.to_string())),
        };
        self.publish(ConnectionState::Connecting {
            server: choice.clone(),
        });
        let name = format!("Proton VPN {}", server.name);

        let (keys, certificate) = match self.keyring.valid(&self.client, &self.settings).await {
            Ok(credentials) => credentials,
            Err(error) => return Outcome::Failed(Problem::Certificate(error.to_string())),
        };
        let session = ServerSession {
            name,
            peer: server.name.clone(),
            choice,
            label: entry.label().map(ToOwned::to_owned),
            entry,
            ports,
            passphrase: (self.protocol.vpn() == VpnProtocol::OpenVpn)
                .then(OpenVpnConfig::generate_passphrase),
            keys,
            certificate,
            status: None,
        };
        let plan = match self.plan(&session) {
            Ok(plan) => plan,
            Err(error) => return Outcome::NextServer(Problem::Tunnel(error.to_string())),
        };
        let (tunnel, mut events) = match self.tunnels.open(&session.name, &plan).await {
            Ok(opened_tunnel) => opened_tunnel,
            Err(error) => return Outcome::NextServer(Problem::Tunnel(error.to_string())),
        };

        let outcome = match self.wait_until_up(&mut events).await {
            Some(outcome) => outcome,
            None => {
                self.supervise_connection(session, &tunnel, &mut events)
                    .await
            }
        };
        drop(events);
        if let Err(error) = self.tunnels.close(tunnel).await {
            tracing::warn!(%error, "failed to close VPN tunnel");
        }
        outcome
    }

    fn plan(&self, session: &ServerSession) -> Result<TunnelPlan> {
        let ipv6 = self.settings.uses_ipv6();
        if let Some(passphrase) = &session.passphrase {
            return Ok(TunnelPlan::OpenVpn(
                session
                    .entry
                    .openvpn(
                        &session.certificate,
                        &session.keys,
                        passphrase,
                        session.ports.clone(),
                    )?
                    .ipv6(ipv6),
            ));
        }
        let secret = session.keys.x25519_secret_key();
        match self.protocol.transmission() {
            Transmission::Udp => Ok(TunnelPlan::WireGuard(
                session
                    .entry
                    .wireguard(&secret, session.choice.port)?
                    .ipv6(ipv6),
            )),
            Transmission::Tcp | Transmission::Tls => Ok(TunnelPlan::Protun(session.entry.protun(
                session.peer.clone(),
                &secret,
                session.ports.clone(),
            )?)),
        }
    }

    async fn rewrite_certificate_files(&self, session: &ServerSession, tunnel: &T::Tunnel) {
        if session.passphrase.is_none() {
            return;
        }
        let renewed = match self.plan(session) {
            Ok(plan) => self.tunnels.renew(tunnel, &session.name, &plan).await,
            Err(error) => Err(error),
        };
        if let Err(error) = renewed {
            tracing::warn!(%error, "failed to renew OpenVPN certificate");
        }
    }

    async fn next_controls(&mut self) -> Option<Controls> {
        self.controls.changed().await.ok()?;
        let latest = self.controls.borrow_and_update().clone();
        (!latest.disconnect).then_some(latest)
    }

    async fn wait_until_up(&mut self, events: &mut TunnelEvents) -> Option<Outcome> {
        let deadline = tokio::time::sleep(TUNNEL_UP_TIMEOUT);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                controls = self.next_controls() => match controls {
                    None => return Some(Outcome::Disconnected),
                    Some(controls) => {
                        self.settings = controls.settings;
                        self.connectivity = controls.connectivity;
                        self.status_requests = controls.status_requests;
                    }
                },
                event = events.next() => match event {
                    Some(TunnelEvent::Up) => return None,
                    Some(TunnelEvent::Connecting) => {}
                    Some(TunnelEvent::Down { .. }) | None => {
                        return Some(Outcome::NextServer(Problem::Tunnel(
                            "tunnel failed to come up".to_owned(),
                        )));
                    }
                },
                () = &mut deadline => {
                    return Some(Outcome::NextServer(Problem::Tunnel(
                        "tunnel timed out coming up".to_owned(),
                    )));
                }
            }
        }
    }

    async fn supervise_connection(
        &mut self,
        mut session: ServerSession,
        tunnel: &T::Tunnel,
        events: &mut TunnelEvents,
    ) -> Outcome {
        let mut agent = match self.connect_agent(&session) {
            Ok(agent) => agent,
            Err(error) => return Outcome::Failed(Problem::Certificate(error.to_string())),
        };
        let mut installed_certificate = session.certificate.certificate.clone();
        let mut refresh_at =
            Instant::now() + session.certificate.time_until_refresh_at(Timestamp::now());

        loop {
            let mut refresh = false;
            tokio::select! {
                controls = self.next_controls() => {
                    let Some(controls) = controls else {
                        return Outcome::Disconnected;
                    };
                    self.apply_controls(controls, &agent, &session);
                }
                event = events.next() => match event {
                    Some(TunnelEvent::Down { deliberate: true }) => {
                        return Outcome::Failed(Problem::ClosedExternally);
                    }
                    Some(TunnelEvent::Down { deliberate: false }) | None => {
                        return Outcome::Reopen(Problem::Tunnel("tunnel went down".to_owned()));
                    }
                    Some(TunnelEvent::Up | TunnelEvent::Connecting) => {}
                },
                event = agent.next_event() => match self.handle_agent_event(event, &mut session).await {
                    ControlFlow::Break(outcome) => return outcome,
                    ControlFlow::Continue(needs_refresh) => refresh = needs_refresh,
                },
                () = tokio::time::sleep_until(refresh_at) => {
                    match self.keyring.valid(&self.client, &self.settings).await {
                        Ok((keys, certificate)) => {
                            refresh = certificate.certificate != session.certificate.certificate;
                            session.keys = keys;
                            session.certificate = certificate;
                            refresh_at = Instant::now()
                                + session.certificate.time_until_refresh_at(Timestamp::now());
                        }
                        Err(error) => {
                            tracing::warn!(%error, "failed to refresh VPN certificate");
                            refresh_at = Instant::now()
                                + retry_after_failure(&session.certificate, Timestamp::now());
                        }
                    }
                }
            }

            if refresh {
                if session.certificate.certificate != installed_certificate {
                    self.rewrite_certificate_files(&session, tunnel).await;
                    installed_certificate.clone_from(&session.certificate.certificate);
                }
                refresh_at =
                    Instant::now() + session.certificate.time_until_refresh_at(Timestamp::now());
                agent = match self.connect_agent(&session) {
                    Ok(agent) => agent,
                    Err(error) => return Outcome::Failed(Problem::Certificate(error.to_string())),
                };
            }
        }
    }

    fn apply_controls(
        &mut self,
        controls: Controls,
        agent: &AgentConnection,
        session: &ServerSession,
    ) {
        if controls.settings != self.settings {
            agent.set_features(controls.settings.agent_features(session.label.as_deref()));
            self.settings = controls.settings;
        }
        if controls.status_requests != self.status_requests {
            self.status_requests = controls.status_requests;
            agent.request_status(controls.statistics);
        }
        if controls.connectivity != self.connectivity {
            self.connectivity = controls.connectivity;
            agent.set_connectivity(controls.connectivity);
        }
    }

    async fn handle_agent_event(
        &mut self,
        event: Option<AgentEvent>,
        session: &mut ServerSession,
    ) -> ControlFlow<Outcome, bool> {
        match event {
            None => ControlFlow::Continue(true),
            Some(AgentEvent::Status(status)) => {
                session.status = Some(status);
                if self.state.borrow().is_connected() {
                    self.publish_connected(session);
                }
                ControlFlow::Continue(false)
            }
            Some(AgentEvent::State(state)) => match self.handle_agent_state(state, session) {
                AgentStep::Continue => ControlFlow::Continue(false),
                AgentStep::Exit(outcome) => ControlFlow::Break(outcome),
                AgentStep::RefreshCertificate => self.replace_certificate(session).await,
            },
            Some(AgentEvent::Error(error)) => match Reaction::for_error(&error) {
                Reaction::Fail(problem) => ControlFlow::Break(Outcome::Failed(problem)),
                Reaction::RenewKey => ControlFlow::Break(
                    match self.keyring.renew_key(&self.client, &self.settings).await {
                        Ok(_) => Outcome::Renewed,
                        Err(error) => Outcome::Failed(Problem::Certificate(error.to_string())),
                    },
                ),
                Reaction::RefreshCertificate => self.replace_certificate(session).await,
                Reaction::Wait => ControlFlow::Continue(false),
            },
        }
    }

    async fn replace_certificate(
        &mut self,
        session: &mut ServerSession,
    ) -> ControlFlow<Outcome, bool> {
        match self
            .keyring
            .replacement_for(&self.client, &self.settings, &session.certificate)
            .await
        {
            Ok((keys, certificate)) => {
                session.keys = keys;
                session.certificate = certificate;
                ControlFlow::Continue(true)
            }
            Err(error) => {
                ControlFlow::Break(Outcome::Failed(Problem::Certificate(error.to_string())))
            }
        }
    }

    fn handle_agent_state(&mut self, state: AgentState, session: &ServerSession) -> AgentStep {
        match state {
            AgentState::Connected => {
                self.tracker.reset(true);
                self.publish_connected(session);
            }
            AgentState::ConnectionError | AgentState::ServerUnreachable => {
                match self.tracker.record_unreachable() {
                    UnreachableAction::SilentReconnect => {}
                    UnreachableAction::Fallback => {
                        self.tracker.record_fallback();
                        return AgentStep::Exit(Outcome::NextServer(Problem::Unreachable));
                    }
                    UnreachableAction::Error => self.publish(ConnectionState::Interrupted {
                        server: session.choice.clone(),
                        problem: Problem::Unreachable,
                    }),
                }
            }
            AgentState::ClientCertificateExpired | AgentState::ClientCertificateUnknownCa => {
                self.publish_connecting(session);
                return AgentStep::RefreshCertificate;
            }
            AgentState::ServerCertificateError => {
                return AgentStep::Exit(Outcome::NextServer(Problem::PeerAuthFailed));
            }
            AgentState::WaitingForNetwork => self.publish(ConnectionState::WaitingForNetwork {
                server: session.choice.clone(),
            }),
            AgentState::HardJailed | AgentState::SoftJailed => self.publish_connecting(session),
            AgentState::Connecting if !self.tracker.is_silent() => self.publish_connecting(session),
            AgentState::Connecting | AgentState::Disconnected => {}
        }
        AgentStep::Continue
    }

    fn connect_agent(&self, session: &ServerSession) -> Result<AgentConnection> {
        let mut agent =
            LocalAgent::new(&session.certificate, &session.keys, session.entry.domain())
                .features(self.settings.agent_features(session.label.as_deref()))
                .connectivity(self.connectivity);
        if let Some(address) = self.agent.address {
            agent = agent.address(address);
        }
        if let Some(roots) = &self.agent.roots {
            agent = agent.roots(roots.clone());
        }
        agent.connect()
    }

    fn publish_connected(&self, session: &ServerSession) {
        self.publish(ConnectionState::Connected {
            server: session.choice.clone(),
            status: session.status.clone(),
        });
    }

    fn publish_connecting(&self, session: &ServerSession) {
        self.publish(ConnectionState::Connecting {
            server: session.choice.clone(),
        });
    }

    fn publish(&self, state: ConnectionState) {
        self.state.send_if_modified(|current_state| {
            let changed = *current_state != state;
            *current_state = state;
            changed
        });
    }
}

pub(crate) struct SelectedServer {
    pub(crate) entry: ServerEntry,
    pub(crate) choice: ServerChoice,
    pub(crate) ports: Vec<u16>,
}

pub(crate) fn select_server(
    server: &LogicalServer,
    protocol: Protocol,
    defaults: &DefaultPorts,
) -> Result<SelectedServer> {
    let entry = server.entry(protocol)?;
    let ports = match entry.ports().is_empty() {
        true => defaults.for_protocol(protocol).to_vec(),
        false => entry.ports().to_vec(),
    };
    let port = *ports.first().ok_or_else(|| Error::NoPort {
        server: server.name.clone(),
        protocol,
    })?;
    let choice = ServerChoice::new(server, &entry, port);
    Ok(SelectedServer {
        entry,
        choice,
        ports,
    })
}
