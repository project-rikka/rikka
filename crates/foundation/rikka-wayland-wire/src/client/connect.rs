use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::client::error::{Error, Result};

const INHERITED: &str = "WAYLAND_SOCKET";
const DISPLAY: &str = "WAYLAND_DISPLAY";

static INHERITED_TAKEN: AtomicBool = AtomicBool::new(false);

pub(crate) fn from_env() -> Result<OwnedFd> {
    if let Some(inherited) = std::env::var_os(INHERITED) {
        let raw: RawFd = inherited
            .to_str()
            .and_then(|text| text.parse().ok())
            .ok_or(Error::InheritedSocket)?;
        if INHERITED_TAKEN.swap(true, Ordering::AcqRel) {
            return Err(Error::InheritedSocket);
        }
        let socket = unsafe { OwnedFd::from_raw_fd(raw) };
        rustix::io::fcntl_setfd(&socket, rustix::io::FdFlags::CLOEXEC)
            .map_err(|_| Error::InheritedSocket)?;
        return Ok(socket);
    }
    let display = std::env::var_os(DISPLAY)
        .filter(|display| !display.is_empty())
        .ok_or(Error::NoDisplay)?;
    to(PathBuf::from(display))
}

pub(crate) fn to(display: PathBuf) -> Result<OwnedFd> {
    let path = if display.is_absolute() {
        display
    } else {
        rikka_dirs::runtime_dir()
            .ok_or(Error::NoRuntimeDir)?
            .join(display)
    };
    UnixStream::connect(&path)
        .map(OwnedFd::from)
        .map_err(|source| Error::Connect {
            path,
            source: Arc::new(source),
        })
}
