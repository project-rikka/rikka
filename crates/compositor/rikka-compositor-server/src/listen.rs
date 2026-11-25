use std::fmt;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use rustix::fs::{FlockOperation, Mode, OFlags};
use rustix::io::Errno;

use crate::display::DisplayHandle;
use crate::error::{Error, Result};
use crate::options::ClientOptions;

const SOCKET_PATH_LIMIT: usize = 107;
const AUTOMATIC_NAMES: u32 = 32;

pub struct Listener {
    stop: Option<Box<dyn FnOnce()>>,
}

impl Listener {
    pub(crate) fn new(stop: impl FnOnce() + 'static) -> Self {
        Self {
            stop: Some(Box::new(stop)),
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop();
        }
    }
}

impl fmt::Debug for Listener {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Listener")
            .field("accepting", &self.stop.is_some())
            .finish()
    }
}

#[derive(Debug)]
pub struct Socket {
    name: String,
    path: PathBuf,
    lock_path: PathBuf,
    _listener: Listener,
    _lock: OwnedFd,
}

impl Socket {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(&self.lock_path);
    }
}

pub(crate) fn socket<S: 'static>(display: &DisplayHandle<S>, name: Option<&str>) -> Result<Socket> {
    if let Some(name) = name {
        return bind(display, name, resolve(name)?, ClientOptions::new());
    }
    for number in 0..=AUTOMATIC_NAMES {
        let name = format!("wayland-{number}");
        match bind(display, &name, resolve(&name)?, ClientOptions::new()) {
            Err(Error::SocketLocked(_)) => continue,
            result => return result,
        }
    }
    Err(Error::NoFreeSocket)
}

pub(crate) fn socket_at<S: 'static>(
    display: &DisplayHandle<S>,
    path: &Path,
    options: ClientOptions,
) -> Result<Socket> {
    bind(display, &path.to_string_lossy(), path.to_owned(), options)
}

fn resolve(name: &str) -> Result<PathBuf> {
    if name.starts_with('/') {
        return Ok(PathBuf::from(name));
    }
    Ok(rikka_dirs::runtime_dir()
        .ok_or(Error::NoRuntimeDir)?
        .join(name))
}

pub(crate) fn accept_all(listener: &UnixListener, mut admit: impl FnMut(UnixStream)) {
    loop {
        match listener.accept() {
            Ok((stream, _)) => admit(stream),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                tracing::warn!("failed to accept client: {error}");
                break;
            }
        }
    }
}

fn bind<S: 'static>(
    display: &DisplayHandle<S>,
    name: &str,
    path: PathBuf,
    options: ClientOptions,
) -> Result<Socket> {
    if path.as_os_str().len() > SOCKET_PATH_LIMIT {
        return Err(Error::PathTooLong(path));
    }
    let mut lock_path = path.clone().into_os_string();
    lock_path.push(".lock");
    let lock_path = PathBuf::from(lock_path);
    let lock = rustix::fs::open(
        &lock_path,
        OFlags::CREATE | OFlags::CLOEXEC | OFlags::RDWR,
        Mode::RUSR | Mode::WUSR | Mode::RGRP | Mode::WGRP,
    )
    .map_err(|error| io_error(format!("opening {}", lock_path.display()), error))?;
    match rustix::fs::flock(&lock, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => {}
        Err(Errno::WOULDBLOCK) => return Err(Error::SocketLocked(path)),
        Err(error) => return Err(io_error(format!("locking {}", lock_path.display()), error)),
    }
    match rustix::fs::lstat(&path) {
        Ok(stat) if stat.st_mode & (Mode::WUSR.bits() | Mode::WGRP.bits()) != 0 => {
            let _ = std::fs::remove_file(&path);
        }
        Ok(_) | Err(Errno::NOENT) => {}
        Err(error) => return Err(io_error(format!("inspecting {}", path.display()), error)),
    }
    let listener =
        UnixListener::bind(&path).map_err(Error::io(format!("binding {}", path.display())))?;
    let listener = match display.accept_on(listener, options) {
        Ok(listener) => listener,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
    };
    Ok(Socket {
        name: name.to_owned(),
        path,
        lock_path,
        _listener: listener,
        _lock: lock,
    })
}

fn io_error(action: String, error: Errno) -> Error {
    Error::Io {
        action,
        source: error.into(),
    }
}
