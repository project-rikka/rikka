use std::ffi::OsString;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::{FdFlags, fcntl_dupfd_cloexec, fcntl_setfd};
use rustix::pipe::{PipeFlags, pipe_with};
use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};

use crate::display_slot::DisplaySlot;
use crate::error::{Error, Result};

const PROGRAM: &str = "Xwayland";
const KEPT_ENVIRONMENT: [&str; 3] = ["PATH", "XDG_RUNTIME_DIR", "DBUS_SYSTEM_BUS_ADDRESS"];
const LOWEST_INHERITED_FD: RawFd = 3;

#[derive(Debug)]
#[must_use]
pub struct XwaylandLaunch {
    program: PathBuf,
}

impl XwaylandLaunch {
    pub fn program(mut self, program: impl Into<PathBuf>) -> Self {
        self.program = program.into();
        self
    }

    pub fn spawn(self) -> Result<(Xwayland, UnixStream)> {
        let mut slot = DisplaySlot::claim()?;
        let (wayland, wayland_for_child) = UnixStream::pair().map_err(Error::Spawn)?;
        let (wm, wm_for_child) = UnixStream::pair().map_err(Error::Spawn)?;
        let (readiness, readiness_for_child) =
            pipe_with(PipeFlags::CLOEXEC).map_err(|error| Error::Spawn(error.into()))?;
        let flags = fcntl_getfl(&readiness).map_err(|error| Error::Spawn(error.into()))?;
        fcntl_setfl(&readiness, flags | OFlags::NONBLOCK)
            .map_err(|error| Error::Spawn(error.into()))?;

        let inherit = |fd: BorrowedFd<'_>| {
            fcntl_dupfd_cloexec(fd, LOWEST_INHERITED_FD).map_err(|error| Error::Spawn(error.into()))
        };
        let listeners = slot
            .sockets()
            .iter()
            .map(|socket| inherit(socket.as_fd()))
            .collect::<Result<Vec<OwnedFd>>>()?;
        let wayland_for_child = inherit(wayland_for_child.as_fd())?;
        let wm_for_child = inherit(wm_for_child.as_fd())?;
        let readiness_for_child = inherit(readiness_for_child.as_fd())?;

        let mut command = Command::new(&self.program);
        command
            .arg(format!(":{}", slot.number()))
            .args(["-rootless", "-terminate"]);
        for listener in &listeners {
            command
                .arg("-listenfd")
                .arg(listener.as_raw_fd().to_string());
        }
        command
            .arg("-wm")
            .arg(wm_for_child.as_raw_fd().to_string())
            .arg("-displayfd")
            .arg(readiness_for_child.as_raw_fd().to_string())
            .env_clear()
            .envs(kept_environment())
            .env("WAYLAND_SOCKET", wayland_for_child.as_raw_fd().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let inherited: Vec<RawFd> = listeners
            .iter()
            .chain([&wayland_for_child, &wm_for_child, &readiness_for_child])
            .map(AsRawFd::as_raw_fd)
            .collect();
        unsafe {
            command.pre_exec(move || {
                for fd in &inherited {
                    fcntl_setfd(BorrowedFd::borrow_raw(*fd), FdFlags::empty())?;
                }
                Ok(())
            });
        }

        let mut child = command.spawn().map_err(Error::Spawn)?;
        let pidfd = match pidfd_open(Pid::from_child(&child), PidfdFlags::NONBLOCK) {
            Ok(pidfd) => pidfd,
            Err(error) => {
                if let Err(error) = child.kill().and_then(|()| child.wait().map(drop)) {
                    tracing::warn!("failed to kill Xwayland after readiness watch failed: {error}");
                }
                return Err(Error::Spawn(error.into()));
            }
        };
        slot.release_sockets();
        tracing::info!("starting Xwayland on :{}", slot.number());
        Ok((
            Xwayland {
                slot,
                child: Some(child),
                status: None,
                pidfd,
                readiness,
                wm: Some(wm),
                announced: Vec::new(),
            },
            wayland,
        ))
    }
}

#[derive(Debug)]
pub struct Xwayland {
    slot: DisplaySlot,
    child: Option<Child>,
    status: Option<ExitStatus>,
    pidfd: OwnedFd,
    readiness: OwnedFd,
    wm: Option<UnixStream>,
    announced: Vec<u8>,
}

impl Xwayland {
    pub fn launch() -> XwaylandLaunch {
        XwaylandLaunch {
            program: PathBuf::from(PROGRAM),
        }
    }

    #[must_use]
    pub fn display_number(&self) -> u32 {
        self.slot.number()
    }

    #[must_use]
    pub fn display(&self) -> String {
        format!(":{}", self.slot.number())
    }

    #[must_use]
    pub fn ready_fd(&self) -> BorrowedFd<'_> {
        self.readiness.as_fd()
    }

    #[must_use]
    pub fn exit_fd(&self) -> BorrowedFd<'_> {
        self.pidfd.as_fd()
    }

    pub fn ready(&mut self) -> Result<Option<UnixStream>> {
        if self.wm.is_none() {
            return Ok(None);
        }
        let mut buffer = [0u8; 16];
        loop {
            match rustix::io::read(&self.readiness, &mut buffer) {
                Ok(0) => return Err(Error::ExitedEarly),
                Ok(length) => {
                    self.announced.extend_from_slice(&buffer[..length]);
                    if self.announced.contains(&b'\n') {
                        tracing::info!("Xwayland ready on {}", self.display());
                        return Ok(self.wm.take());
                    }
                }
                Err(rustix::io::Errno::AGAIN) => return Ok(None),
                Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(Error::Readiness(error.into())),
            }
        }
    }

    pub fn exited(&mut self) -> Option<ExitStatus> {
        if self.status.is_none()
            && let Some(child) = self.child.as_mut()
        {
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.status = Some(status);
                    self.child = None;
                }
                Ok(None) => {}
                Err(error) => tracing::warn!("failed to reap Xwayland: {error}"),
            }
        }
        self.status
    }
}

impl Drop for Xwayland {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        if let Err(error) = pidfd_send_signal(&self.pidfd, Signal::TERM) {
            tracing::warn!("failed to signal Xwayland: {error}");
        }
        let reaper = std::thread::Builder::new()
            .name("rikka-xwayland-reaper".into())
            .spawn(move || {
                if let Err(error) = child.wait() {
                    tracing::warn!("failed to reap Xwayland: {error}");
                }
            });
        if let Err(error) = reaper {
            tracing::warn!("failed to reap Xwayland, leaving it to the session: {error}");
        }
    }
}

fn kept_environment() -> Vec<(&'static str, OsString)> {
    KEPT_ENVIRONMENT
        .into_iter()
        .filter_map(|key| std::env::var_os(key).map(|value| (key, value)))
        .collect()
}
