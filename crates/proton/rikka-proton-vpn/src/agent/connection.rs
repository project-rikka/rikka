use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::Arc;
use std::time::{Duration, Instant};

use jiff::Timestamp;
use rustix::net::sockopt;
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncWrite, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use crate::agent::backoff::Backoff;
use crate::agent::code::AgentCode;
use crate::agent::event::{AgentError, AgentEvent, AgentState};
use crate::agent::features::AgentFeatures;
use crate::agent::status::{AgentStatus, JailState};
use crate::agent::tls::{self, Failure, PROTON_ROOTS};
use crate::agent::wire::{self, Inbound};
use crate::certificate::Certificate;
use crate::error::{Error, Result};
use crate::key::KeyPair;

const AGENT_ADDRESS: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(10, 2, 0, 1), 65432));
const DIAL_TIMEOUT: Duration = Duration::from_secs(5);
const KEEPALIVE: Duration = Duration::from_secs(60);
const KEEPALIVE_PROBES: u32 = 2;
const EVENT_CAPACITY: usize = 64;
const INCOMING_CAPACITY: usize = 8;

#[must_use]
#[derive(Debug)]
pub struct LocalAgent {
    certificate: String,
    keys: KeyPair,
    expires_at: Timestamp,
    server_name: String,
    address: SocketAddr,
    roots: String,
    features: AgentFeatures,
    connectivity: bool,
}

#[derive(Debug)]
pub struct AgentConnection {
    controls: watch::Sender<Controls>,
    events: mpsc::Receiver<AgentEvent>,
    state: watch::Receiver<AgentState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Controls {
    features: AgentFeatures,
    connectivity: bool,
    status_requests: u64,
    statistics: bool,
}

#[derive(Debug, Default)]
struct ControlChange {
    features: Option<AgentFeatures>,
    connectivity: Option<bool>,
    status_request: Option<bool>,
}

enum SessionEnd {
    Closed,
    Offline,
    Failed(io::Error),
}

struct Session {
    requested: AgentFeatures,
    status: Option<AgentStatus>,
    features_sent: bool,
}

struct Worker {
    config: Arc<ClientConfig>,
    server_name: ServerName<'static>,
    address: SocketAddr,
    expires_at: Timestamp,
    connectivity: bool,
    controls: watch::Receiver<Controls>,
    applied: Controls,
    events: mpsc::Sender<AgentEvent>,
    state: watch::Sender<AgentState>,
    session: Session,
}

impl LocalAgent {
    pub fn new(
        certificate: &Certificate,
        keys: &KeyPair,
        server_name: impl Into<String>,
    ) -> LocalAgent {
        LocalAgent {
            certificate: certificate.certificate.clone(),
            keys: keys.clone(),
            expires_at: certificate.expires_at,
            server_name: server_name.into(),
            address: AGENT_ADDRESS,
            roots: PROTON_ROOTS.to_owned(),
            features: AgentFeatures::default(),
            connectivity: true,
        }
    }

    pub fn address(mut self, address: SocketAddr) -> LocalAgent {
        self.address = address;
        self
    }

    pub fn roots(mut self, pem: impl Into<String>) -> LocalAgent {
        self.roots = pem.into();
        self
    }

    pub fn features(mut self, features: AgentFeatures) -> LocalAgent {
        self.features = features;
        self
    }

    pub fn connectivity(mut self, available: bool) -> LocalAgent {
        self.connectivity = available;
        self
    }

    pub fn connect(self) -> Result<AgentConnection> {
        let config = tls::client_config(&self.certificate, &self.keys, &self.roots)?;
        let server_name = ServerName::try_from(self.server_name.clone()).map_err(|_| {
            Error::InvalidServerName {
                name: self.server_name.clone(),
            }
        })?;

        let initial = Controls {
            features: self.features.clone(),
            connectivity: self.connectivity,
            status_requests: 0,
            statistics: false,
        };
        let (controls, control_receiver) = watch::channel(initial.clone());
        let (event_sender, events) = mpsc::channel(EVENT_CAPACITY);
        let (state, state_receiver) = watch::channel(AgentState::Connecting);
        let worker = Worker {
            config,
            server_name,
            address: self.address,
            expires_at: self.expires_at,
            connectivity: self.connectivity,
            controls: control_receiver,
            applied: initial,
            events: event_sender,
            state,
            session: Session {
                requested: self.features,
                status: None,
                features_sent: false,
            },
        };
        tokio::spawn(worker.run());

        Ok(AgentConnection {
            controls,
            events,
            state: state_receiver,
        })
    }
}

impl AgentConnection {
    #[must_use]
    pub fn state(&self) -> AgentState {
        *self.state.borrow()
    }

    pub async fn next_event(&mut self) -> Option<AgentEvent> {
        self.events.recv().await
    }

    pub fn set_features(&self, features: AgentFeatures) {
        self.controls.send_if_modified(|controls| {
            let changed = controls.features != features;
            controls.features = features;
            changed
        });
    }

    pub fn request_status(&self, statistics: bool) {
        self.controls.send_modify(|controls| {
            controls.status_requests += 1;
            controls.statistics = statistics;
        });
    }

    pub fn set_connectivity(&self, available: bool) {
        self.controls.send_if_modified(|controls| {
            let changed = controls.connectivity != available;
            controls.connectivity = available;
            changed
        });
    }
}

impl Worker {
    async fn run(mut self) {
        let mut backoff = Backoff::new();

        loop {
            while !self.connectivity {
                self.set_state(AgentState::WaitingForNetwork);
                if !self.await_control_change().await {
                    return self.finish();
                }
            }

            let started = Instant::now();
            self.set_state(AgentState::Connecting);
            let ended = match self.dial().await {
                Ok(stream) => self.run_session(stream).await,
                Err(error) => SessionEnd::Failed(error),
            };

            match ended {
                SessionEnd::Closed => return self.finish(),
                SessionEnd::Offline => continue,
                SessionEnd::Failed(error) => {
                    tracing::debug!(%error, "local agent connection failed");
                    let state = self.failure_state(&error);
                    self.set_state(state);
                    if state.is_terminal() {
                        while self.await_control_change().await {}
                        return self.finish();
                    }
                }
            }

            let wait = backoff.next_delay(started.elapsed());
            if !self.connectivity {
                continue;
            }
            let delay = tokio::time::sleep(wait);
            tokio::pin!(delay);
            loop {
                tokio::select! {
                    () = &mut delay => break,
                    change = self.next_control_change() => match change {
                        None => return self.finish(),
                        Some(change) => {
                            self.apply_change(change);
                            if !self.connectivity {
                                break;
                            }
                        }
                    },
                }
            }
        }
    }

    fn failure_state(&self, error: &io::Error) -> AgentState {
        match tls::classify(error) {
            Failure::ServerCertificate => AgentState::ServerCertificateError,
            Failure::ClientCertificateExpired => AgentState::ClientCertificateExpired,
            Failure::ClientCertificateUnknownCa => AgentState::ClientCertificateUnknownCa,
            Failure::Unreachable => AgentState::ServerUnreachable,
            Failure::Other if Timestamp::now() >= self.expires_at => {
                AgentState::ClientCertificateExpired
            }
            Failure::Other => AgentState::ConnectionError,
        }
    }

    async fn dial(&self) -> io::Result<TlsStream<TcpStream>> {
        let connecting = async {
            let stream = TcpStream::connect(self.address).await?;
            sockopt::set_socket_keepalive(&stream, true)?;
            sockopt::set_tcp_keepidle(&stream, KEEPALIVE)?;
            sockopt::set_tcp_keepintvl(&stream, KEEPALIVE)?;
            sockopt::set_tcp_keepcnt(&stream, KEEPALIVE_PROBES)?;
            TlsConnector::from(Arc::clone(&self.config))
                .connect(self.server_name.clone(), stream)
                .await
        };
        tokio::time::timeout(DIAL_TIMEOUT, connecting)
            .await
            .map_err(|_| {
                io::Error::new(io::ErrorKind::TimedOut, "local agent connection timed out")
            })?
    }

    async fn run_session(&mut self, stream: TlsStream<TcpStream>) -> SessionEnd {
        let (reader, mut writer) = tokio::io::split(stream);
        let (sender, mut incoming) = mpsc::channel(INCOMING_CAPACITY);
        let listener = tokio::spawn(read_messages(reader, sender));
        self.session.status = None;
        self.session.features_sent = false;

        let ended = loop {
            tokio::select! {
                change = self.next_control_change() => {
                    let Some(change) = change else {
                        break SessionEnd::Closed;
                    };
                    if let Some(features) = &change.features {
                        self.session.requested.merge(features);
                        if let Err(error) = self.send_unsent_features(&mut writer).await {
                            break SessionEnd::Failed(error);
                        }
                    }
                    if let Some(statistics) = change.status_request
                        && let Err(error) = wire::request_status(&mut writer, statistics).await
                    {
                        break SessionEnd::Failed(error);
                    }
                    if let Some(available) = change.connectivity {
                        self.connectivity = available;
                        if !available {
                            break SessionEnd::Offline;
                        }
                    }
                }
                message = incoming.recv() => match message {
                    Some(Ok(message)) => {
                        if let Err(error) = self.handle_message(&message, &mut writer).await {
                            break SessionEnd::Failed(error);
                        }
                    }
                    Some(Err(error)) => break SessionEnd::Failed(error),
                    None => break SessionEnd::Failed(io::Error::from(io::ErrorKind::UnexpectedEof)),
                },
            }
        };

        listener.abort();
        ended
    }

    async fn handle_message(
        &mut self,
        message: &[u8],
        writer: &mut WriteHalf<TlsStream<TcpStream>>,
    ) -> io::Result<()> {
        for inbound in wire::parse(message)? {
            match inbound {
                Inbound::Status(status) => {
                    self.apply_status(AgentStatus::from(*status));
                    if !self.session.features_sent {
                        self.send_unsent_features(writer).await?;
                        self.session.features_sent = true;
                    }
                }
                Inbound::Error(error) => {
                    let is_final = self
                        .session
                        .status
                        .as_ref()
                        .and_then(|status| status.reason.as_ref())
                        .is_some_and(|reason| reason.is_final);
                    self.notify(AgentEvent::Error(AgentError {
                        code: AgentCode::from_code(error.code),
                        description: error.description,
                        is_final,
                    }));
                }
            }
        }
        Ok(())
    }

    fn apply_status(&mut self, mut status: AgentStatus) {
        let partner_jail = status.state == JailState::HardJailed
            && status
                .reason
                .as_ref()
                .is_some_and(|reason| reason.code == AgentCode::PolicyViolationLowPlan)
            && self.session.requested.bounces_through_partner();
        if partner_jail {
            status.state = JailState::Connected;
            status.reason = None;
        }

        match status.state {
            JailState::Jailed => self.set_state(AgentState::SoftJailed),
            JailState::HardJailed => self.set_state(AgentState::HardJailed),
            JailState::Connected => self.set_state(AgentState::Connected),
            JailState::Other(_) => {}
        }

        let error = match &status.reason {
            Some(reason) => Some(AgentError {
                code: reason.code,
                description: reason.description.clone(),
                is_final: reason.is_final,
            }),
            None if status.state == JailState::HardJailed => Some(AgentError {
                code: AgentCode::Unknown,
                description: String::new(),
                is_final: false,
            }),
            None => None,
        };

        self.session.status = Some(status.clone());
        self.notify(AgentEvent::Status(Box::new(status)));
        if let Some(error) = error {
            self.notify(AgentEvent::Error(error));
        }
    }

    async fn send_unsent_features<W: AsyncWrite + Unpin>(
        &mut self,
        writer: &mut W,
    ) -> io::Result<()> {
        let Some(status) = self.session.status.as_mut() else {
            return Ok(());
        };
        let unsent = self.session.requested.differences_from(&status.features);
        if unsent.is_empty() {
            return Ok(());
        }
        status.features.clear_sent(&unsent);
        wire::send_features(writer, &unsent).await
    }

    async fn await_control_change(&mut self) -> bool {
        match self.next_control_change().await {
            Some(change) => {
                self.apply_change(change);
                true
            }
            None => false,
        }
    }

    async fn next_control_change(&mut self) -> Option<ControlChange> {
        self.controls.changed().await.ok()?;
        let latest = self.controls.borrow_and_update().clone();
        let change = ControlChange {
            features: (latest.features != self.applied.features).then(|| latest.features.clone()),
            connectivity: (latest.connectivity != self.applied.connectivity)
                .then_some(latest.connectivity),
            status_request: (latest.status_requests != self.applied.status_requests)
                .then_some(latest.statistics),
        };
        self.applied = latest;
        Some(change)
    }

    fn apply_change(&mut self, change: ControlChange) {
        if let Some(features) = &change.features {
            self.session.requested.merge(features);
        }
        if let Some(available) = change.connectivity {
            self.connectivity = available;
        }
    }

    fn set_state(&self, state: AgentState) {
        let changed = self.state.send_if_modified(|current| {
            let changed = *current != state;
            *current = state;
            changed
        });
        if changed {
            self.notify(AgentEvent::State(state));
        }
    }

    fn notify(&self, event: AgentEvent) {
        match self.events.try_send(event) {
            Ok(()) => {}
            Err(mpsc::error::TrySendError::Full(event)) => {
                tracing::warn!(?event, "local agent event queue full, dropping event");
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!("local agent event receiver dropped");
            }
        }
    }

    fn finish(self) {
        self.set_state(AgentState::Disconnected);
    }
}

async fn read_messages(
    mut reader: ReadHalf<TlsStream<TcpStream>>,
    sender: mpsc::Sender<io::Result<Vec<u8>>>,
) {
    loop {
        let message = wire::read(&mut reader).await;
        let failed = message.is_err();
        if sender.send(message).await.is_err() || failed {
            return;
        }
    }
}
