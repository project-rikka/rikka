use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("XDG_RUNTIME_DIR is not set")]
    NoRuntimeDir,

    #[error("failed to spawn process: {0}")]
    Spawn(#[source] std::io::Error),

    #[error("compositor did not become ready within {} s", .0.as_secs())]
    StartupTimeout(Duration),

    #[error("session has stopped")]
    Stopped,

    #[error("no session is running")]
    NotRunning,

    #[error("the live desktop is neither started nor stopped by the agent")]
    LiveDesktop,

    #[error("session has not published its environment")]
    NoEnvironment,

    #[error("compositor request failed: {0}")]
    Compositor(String),

    #[error("screen capture failed: {0}")]
    Capture(String),

    #[error("input injection failed: {0}")]
    Input(String),

    #[error("({x}, {y}) is outside the {width}x{height} screen")]
    OutOfBounds {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },

    #[error("region {region} is outside the {width}x{height} screen")]
    RegionOutside {
        region: crate::region::Region,
        width: u32,
        height: u32,
    },

    #[error("unknown scroll direction {0:?}")]
    UnknownScrollDirection(String),

    #[error("no command given")]
    EmptyCommand,

    #[error("no matching window appeared within {} ms", .0.as_millis())]
    WindowTimeout(Duration),

    #[error("screen kept changing for {} ms", .0.as_millis())]
    IdleTimeout(Duration),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn capture(error: impl std::fmt::Display) -> Error {
    Error::Capture(error.to_string())
}

pub(crate) fn input(error: impl std::fmt::Display) -> Error {
    Error::Input(error.to_string())
}
