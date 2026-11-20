use std::io;
use std::path::PathBuf;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot open seat (is seatd or logind running?): {0}")]
    SeatUnavailable(io::Error),
    #[error("seat refused to open {path}: {source}")]
    DeviceRefused { path: PathBuf, source: io::Error },
    #[error("failed to close device: {0}")]
    DeviceNotClosed(io::Error),
    #[error("failed to switch to VT {vt}: {source}")]
    VtSwitch { vt: i32, source: io::Error },
    #[error("device requested from the session thread")]
    OpenedFromSessionThread,
    #[error("session thread is gone")]
    SessionGone,
    #[error("failed to register event source: {0}")]
    Reactor(io::Error),
    #[error("failed to read udev: {0}")]
    Udev(io::Error),
    #[error("no GPU on seat {0}")]
    NoGpu(String),
    #[error("atomic modesetting unsupported: {0}")]
    AtomicUnsupported(io::Error),
    #[error("missing required DRM capability {0}")]
    MissingCapability(&'static str),
    #[error("failed to query DRM device: {0}")]
    Query(io::Error),
    #[error("failed to create GBM device: {0}")]
    Gbm(io::Error),
    #[error("object {object} has no {name} property")]
    MissingProperty { object: u32, name: &'static str },
    #[error("no free CRTC for {0}")]
    NoCrtc(String),
    #[error("no free primary plane for {0}")]
    NoPrimaryPlane(String),
    #[error("failed to create DRM lease: {0}")]
    Lease(io::Error),
    #[error("unknown lessee")]
    UnknownLease,
    #[error("{0} is disconnected")]
    Disconnected(String),
    #[error("unknown output {0}")]
    UnknownOutput(String),
    #[error("{0} cannot signal PQ HDR")]
    HdrUnsupported(String),
    #[error("no format and modifier shared by display and renderer")]
    NoScanoutFormat,
    #[error("unknown fourcc {0:#010x}")]
    UnknownFourcc(u32),
    #[error("failed to allocate buffer: {0}")]
    Allocation(io::Error),
    #[error("failed to export buffer: {0}")]
    Export(io::Error),
    #[error("failed to import buffer: {0}")]
    Import(io::Error),
    #[error("failed to add framebuffer: {0}")]
    Framebuffer(io::Error),
    #[error("failed to create property blob: {0}")]
    Blob(io::Error),
    #[error("{connector} rejected every tested configuration: {source}")]
    ConfigurationRejected {
        connector: String,
        source: io::Error,
    },
    #[error("cursor image {width}x{height} exceeds the {limit_width}x{limit_height} cursor plane")]
    CursorTooLarge {
        width: u32,
        height: u32,
        limit_width: u32,
        limit_height: u32,
    },
    #[error("cursor image has {got} bytes, expected {expected}")]
    CursorPixels { got: usize, expected: usize },
    #[error("failed to spawn KMS thread: {0}")]
    Thread(io::Error),
    #[error("KMS thread has stopped")]
    CommitterGone,
}

pub(crate) fn seat_error(errno: impl Into<i32>) -> io::Error {
    io::Error::from_raw_os_error(errno.into())
}
