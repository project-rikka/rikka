use crate::agent::{AgentCode, AgentStatus};
use crate::servers::{LogicalServer, ServerEntry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerChoice {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub entry_ip: String,
    pub port: u16,
    pub exit_country: String,
    pub city: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Problem {
    #[error("maximum simultaneous VPN connections reached")]
    MaxSessions(AgentCode),
    #[error("VPN key is in use on another device")]
    KeyUsedMultipleTimes,
    #[error("server does not allow P2P traffic")]
    TorrentNotAllowed,
    #[error("connection terminated for terms of service violation")]
    BadBehaviour,
    #[error("plan does not include this server")]
    LowPlan,
    #[error("account has unpaid invoices")]
    Delinquent,
    #[error("VPN server error")]
    ServerError,
    #[error("VPN server rejected connection ({code}): {description}")]
    Agent {
        code: AgentCode,
        description: String,
    },
    #[error("VPN server authentication failed")]
    PeerAuthFailed,
    #[error("VPN server unreachable")]
    Unreachable,
    #[error("failed to obtain VPN certificate: {0}")]
    Certificate(String),
    #[error("tunnel failed: {0}")]
    Tunnel(String),
    #[error("tunnel closed externally")]
    ClosedExternally,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connecting {
        server: ServerChoice,
    },
    Connected {
        server: ServerChoice,
        status: Option<Box<AgentStatus>>,
    },
    WaitingForNetwork {
        server: ServerChoice,
    },
    Interrupted {
        server: ServerChoice,
        problem: Problem,
    },
    Failed {
        problem: Problem,
    },
    Disconnected,
}

impl ServerChoice {
    pub(crate) fn new(server: &LogicalServer, entry: &ServerEntry, port: u16) -> ServerChoice {
        ServerChoice {
            id: server.id.clone(),
            name: server.name.clone(),
            domain: entry.domain().to_owned(),
            entry_ip: entry.ip().to_owned(),
            port,
            exit_country: server.exit_country_code(),
            city: server.city.clone(),
        }
    }
}

impl ConnectionState {
    #[must_use]
    pub fn server(&self) -> Option<&ServerChoice> {
        match self {
            ConnectionState::Connecting { server }
            | ConnectionState::Connected { server, .. }
            | ConnectionState::WaitingForNetwork { server }
            | ConnectionState::Interrupted { server, .. } => Some(server),
            ConnectionState::Failed { .. } | ConnectionState::Disconnected => None,
        }
    }

    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected { .. })
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        matches!(
            self,
            ConnectionState::Failed { .. } | ConnectionState::Disconnected
        )
    }
}
