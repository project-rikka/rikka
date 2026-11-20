use std::path::PathBuf;

use crate::active::ActiveState;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SavedTunnel {
    pub(crate) path: String,
    pub(crate) files: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TunnelReason {
    #[default]
    Unknown,
    None,
    UserDisconnected,
    DeviceDisconnected,
    ServiceStopped,
    IpConfigInvalid,
    ConnectTimeout,
    ServiceStartTimeout,
    ServiceStartFailed,
    NoSecrets,
    LoginFailed,
    ConnectionRemoved,
    DependencyFailed,
    DeviceRealizeFailed,
    DeviceRemoved,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TunnelState {
    pub state: ActiveState,
    pub reason: TunnelReason,
}

impl SavedTunnel {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.path
    }
}

impl TunnelReason {
    pub(crate) fn from_code(code: u32) -> TunnelReason {
        match code {
            1 => TunnelReason::None,
            2 => TunnelReason::UserDisconnected,
            3 => TunnelReason::DeviceDisconnected,
            4 => TunnelReason::ServiceStopped,
            5 => TunnelReason::IpConfigInvalid,
            6 => TunnelReason::ConnectTimeout,
            7 => TunnelReason::ServiceStartTimeout,
            8 => TunnelReason::ServiceStartFailed,
            9 => TunnelReason::NoSecrets,
            10 => TunnelReason::LoginFailed,
            11 => TunnelReason::ConnectionRemoved,
            12 => TunnelReason::DependencyFailed,
            13 => TunnelReason::DeviceRealizeFailed,
            14 => TunnelReason::DeviceRemoved,
            _ => TunnelReason::Unknown,
        }
    }

    #[must_use]
    pub fn is_deliberate(self) -> bool {
        matches!(
            self,
            TunnelReason::UserDisconnected | TunnelReason::ConnectionRemoved
        )
    }
}

impl TunnelState {
    pub(crate) fn from_codes(state: u32, reason: u32) -> TunnelState {
        TunnelState {
            state: ActiveState::from_code(state),
            reason: TunnelReason::from_code(reason),
        }
    }

    pub(crate) fn gone() -> TunnelState {
        TunnelState {
            state: ActiveState::Deactivated,
            reason: TunnelReason::ConnectionRemoved,
        }
    }

    #[must_use]
    pub fn is_up(self) -> bool {
        self.state == ActiveState::Activated
    }

    #[must_use]
    pub fn is_down(self) -> bool {
        self.state == ActiveState::Deactivated
    }
}
