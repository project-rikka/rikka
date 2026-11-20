use crate::agent::code::AgentCode;
use crate::agent::status::AgentStatus;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum AgentState {
    #[default]
    Connecting,
    Connected,
    SoftJailed,
    HardJailed,
    ConnectionError,
    ServerUnreachable,
    WaitingForNetwork,
    ServerCertificateError,
    ClientCertificateExpired,
    ClientCertificateUnknownCa,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentError {
    pub code: AgentCode,
    pub description: String,
    pub is_final: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    State(AgentState),
    Status(Box<AgentStatus>),
    Error(AgentError),
}

impl AgentState {
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            AgentState::ServerCertificateError
                | AgentState::ClientCertificateExpired
                | AgentState::ClientCertificateUnknownCa
                | AgentState::Disconnected
        )
    }
}
