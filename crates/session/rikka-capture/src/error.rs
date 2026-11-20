#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no Wayland session to capture: {0}")]
    NoSession(String),

    #[error("compositor does not support capture: {0} is missing")]
    Unsupported(&'static str),

    #[error("no display named \"{0}\"")]
    NoOutput(String),

    #[error("no window with identifier \"{0}\"")]
    NoWindow(String),

    #[error("capture stopped")]
    Stopped,

    #[error("capture failed: {0}")]
    Failed(&'static str),

    #[error("compositor offers no supported capture buffer format")]
    NoFormat,

    #[error("compositor shares no capture buffer this GPU can read")]
    NoSharedBuffer,

    #[error("failed to allocate capture buffer: {0}")]
    Allocation(String),

    #[error(
        "the region {width}x{height}+{x}+{y} does not lie inside the {source_width}x{source_height} source"
    )]
    Region {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        source_width: u32,
        source_height: u32,
    },

    #[error("wayland: {0}")]
    Protocol(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<rikka_wayland::Error> for Error {
    fn from(error: rikka_wayland::Error) -> Self {
        match error {
            rikka_wayland::Error::NoGlobal(interface) => Error::Unsupported(interface),
            rikka_wayland::Error::Io(error) => Error::Io(error),
            rikka_wayland::Error::Protocol(message) => Error::Protocol(message),
            other => Error::NoSession(other.to_string()),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
