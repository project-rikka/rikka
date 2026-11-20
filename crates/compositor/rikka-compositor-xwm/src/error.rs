use std::io;
use std::path::PathBuf;

use crate::selection::SelectionKind;
use crate::window::XWindowId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no free X display between :0 and :32")]
    NoFreeDisplay,
    #[error("cannot create X11 socket directory {}", .path.display())]
    SocketDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to spawn Xwayland")]
    Spawn(#[source] io::Error),
    #[error("Xwayland exited before becoming ready")]
    ExitedEarly,
    #[error("failed to wait for Xwayland readiness")]
    Readiness(#[source] io::Error),
    #[error("window manager handshake failed: {0}")]
    Handshake(String),
    #[error("another X11 window manager is running")]
    WindowManagerRunning,
    #[error("Xwayland lacks the {0} extension")]
    MissingExtension(&'static str),
    #[error("lost connection to Xwayland")]
    Disconnected,
    #[error("Xwayland rejected a request: {0}")]
    Refused(String),
    #[error("failed to poll X11 window manager sources")]
    Poll(#[source] io::Error),
    #[error("failed to open selection transfer")]
    Transfer(#[source] io::Error),
    #[error("unknown X11 window {0}")]
    UnknownWindow(XWindowId),
    #[error("{0} is override-redirect")]
    OverrideRedirect(XWindowId),
    #[error("X11 {kind} selection does not offer {mime_type}")]
    NotOffered {
        kind: SelectionKind,
        mime_type: String,
    },
    #[error("invalid XSETTINGS name {0:?}")]
    SettingName(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
