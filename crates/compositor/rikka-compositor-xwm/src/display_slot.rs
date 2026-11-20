use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write};
use std::os::fd::OwnedFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use rustix::net::{AddressFamily, SocketAddrUnix, SocketFlags, SocketType};
use rustix::process::{Pid, getpid, getuid, test_kill_process};

use crate::error::{Error, Result};

const SOCKET_DIRECTORY: &str = "/tmp/.X11-unix";
const LAST_DISPLAY: u32 = 32;
const LOCK_MODE: u32 = 0o444;
const DIRECTORY_MODE: u32 = 0o755;
const STICKY: u32 = 0o1000;
const BACKLOG: i32 = 1;

#[derive(Debug)]
pub(crate) struct DisplaySlot {
    number: u32,
    sockets: Vec<OwnedFd>,
    socket_file: bool,
}

impl DisplaySlot {
    pub(crate) fn claim() -> Result<Self> {
        prepare_directory(Path::new(SOCKET_DIRECTORY))?;
        for number in 0..=LAST_DISPLAY {
            match Self::try_claim(number) {
                Ok(Some(slot)) => return Ok(slot),
                Ok(None) => {}
                Err(error) => tracing::debug!("X display :{number} is unusable: {error}"),
            }
        }
        Err(Error::NoFreeDisplay)
    }

    pub(crate) fn number(&self) -> u32 {
        self.number
    }

    pub(crate) fn sockets(&self) -> &[OwnedFd] {
        &self.sockets
    }

    pub(crate) fn release_sockets(&mut self) {
        self.sockets.clear();
    }

    fn try_claim(number: u32) -> io::Result<Option<Self>> {
        if !lock(number)? {
            return Ok(None);
        }
        let mut slot = Self {
            number,
            sockets: Vec::new(),
            socket_file: false,
        };
        let path = socket_path(number);
        slot.sockets.push(listen(&SocketAddrUnix::new_abstract_name(
            path.as_os_str().as_encoded_bytes(),
        )?)?);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        slot.socket_file = true;
        slot.sockets.push(listen(&SocketAddrUnix::new(&path)?)?);
        Ok(Some(slot))
    }
}

impl Drop for DisplaySlot {
    fn drop(&mut self) {
        let mut left = vec![lock_path(self.number)];
        if self.socket_file {
            left.push(socket_path(self.number));
        }
        for path in left {
            if let Err(error) = fs::remove_file(&path)
                && error.kind() != io::ErrorKind::NotFound
            {
                tracing::warn!("failed to remove {}: {error}", path.display());
            }
        }
    }
}

fn socket_path(number: u32) -> PathBuf {
    Path::new(SOCKET_DIRECTORY).join(format!("X{number}"))
}

fn lock_path(number: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/.X{number}-lock"))
}

fn lock(number: u32) -> io::Result<bool> {
    let path = lock_path(number);
    for _ in 0..2 {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(LOCK_MODE)
            .open(&path)
        {
            Ok(mut file) => {
                let lock_written = file.write_all(lock_contents(getpid()).as_bytes());
                if let Err(error) = lock_written {
                    fs::remove_file(&path)?;
                    return Err(error);
                }
                return Ok(true);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                if !stale(&path)? {
                    return Ok(false);
                }
                fs::remove_file(&path)?;
                match fs::remove_file(socket_path(number)) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(false)
}

fn lock_contents(pid: Pid) -> String {
    format!("{:>10}\n", pid.as_raw_nonzero())
}

fn stale(path: &Path) -> io::Result<bool> {
    let contents = fs::read_to_string(path)?;
    let Some(pid) = contents.trim().parse::<i32>().ok().and_then(Pid::from_raw) else {
        return Ok(false);
    };
    Ok(test_kill_process(pid) == Err(rustix::io::Errno::SRCH))
}

fn listen(address: &SocketAddrUnix) -> io::Result<OwnedFd> {
    let socket = rustix::net::socket_with(
        AddressFamily::UNIX,
        SocketType::STREAM,
        SocketFlags::CLOEXEC,
        None,
    )?;
    rustix::net::bind(&socket, address)?;
    rustix::net::listen(&socket, BACKLOG)?;
    Ok(socket)
}

fn prepare_directory(path: &Path) -> Result<()> {
    let refuse = |source: io::Error| Error::SocketDirectory {
        path: path.to_owned(),
        source,
    };
    match DirBuilder::new().mode(DIRECTORY_MODE).create(path) {
        Ok(()) => {
            tracing::info!(
                "created {} for this user only; other users cannot start X servers",
                path.display()
            );
            return Ok(());
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(refuse(error)),
    }
    let metadata = fs::symlink_metadata(path).map_err(refuse)?;
    let owner = metadata.uid();
    let trusted_owner = owner == 0 || owner == getuid().as_raw();
    let shared_safely = owner == getuid().as_raw() || metadata.mode() & STICKY != 0;
    if !metadata.is_dir() || !trusted_owner || !shared_safely {
        return Err(refuse(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "it must be a directory owned by root or this user, sticky when shared",
        )));
    }
    Ok(())
}
