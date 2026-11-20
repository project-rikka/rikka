use std::os::fd::OwnedFd;
use std::os::unix::fs::DirBuilderExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rikka_capture::{Screen, ScreenWindow};
use rikka_watch::{Reach, Watches};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::process::{Pid, PidfdFlags, Signal, kill_process_group, pidfd_open};

use crate::address::Address;
use crate::display::Display;
use crate::environment::Environment;
use crate::error::{Error, Result, capture};
use crate::input::InputDriver;
use crate::launcher::Launcher;
use crate::presence::Presence;
use crate::shell::Shell;

const COMPOSITOR: &str = "rikka-compositor";

const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);

const SHUTDOWN_GRACE: Duration = Duration::from_millis(1_500);

const DEFAULT_LOG_FILTER: &str = "warn,rikka_compositor=info";

const INHERITED_SESSION_VARIABLES: [&str; 4] = [
    "WAYLAND_DISPLAY",
    "WAYLAND_SOCKET",
    "DISPLAY",
    rikka_seam::net::SOCKET_ENV,
];

#[derive(Debug)]
pub(crate) struct Session {
    input: InputDriver,
    compositor: Compositor,
    launcher: Launcher,
    address: Address,
    environment: Environment,
    display: Display,
}

impl Session {
    pub(crate) fn start(display: Display, shell: Shell) -> Result<Session> {
        let address = Address::allocate()?;
        let compositor = Compositor::spawn(&address, display, shell)?;
        compositor.wait_until_ready(&address)?;
        let environment = address.environment()?;
        let input = InputDriver::connect(&address.socket(), display.capture_size())?;
        Ok(Session {
            input,
            compositor,
            launcher: Launcher::default(),
            address,
            environment,
            display,
        })
    }

    pub(crate) fn display(&self) -> Display {
        self.display
    }

    pub(crate) fn socket(&self) -> PathBuf {
        self.address.socket()
    }

    pub(crate) fn input(&mut self) -> &mut InputDriver {
        &mut self.input
    }

    pub(crate) fn presence(&self) -> Presence {
        Presence::new(self.environment.clone(), Arc::clone(&self.compositor.pidfd))
    }

    pub(crate) fn windows(&self) -> Result<Vec<ScreenWindow>> {
        let screen = Screen::connect_to(&self.address.socket()).map_err(capture)?;
        Ok(screen.windows())
    }

    pub(crate) fn launch(&mut self, command: &str) -> Result<u32> {
        self.launcher.launch(
            command,
            Some(&self.environment),
            self.compositor.pid.as_raw_pid(),
        )
    }

    pub(crate) fn is_running(&mut self) -> bool {
        self.compositor.is_running()
    }
}

#[derive(Debug)]
struct Compositor {
    child: Child,
    pid: Pid,
    pidfd: Arc<OwnedFd>,
}

impl Compositor {
    fn spawn(address: &Address, display: Display, shell: Shell) -> Result<Compositor> {
        let mut command = Command::new(compositor_program());
        command
            .arg(format!(
                "--headless={}x{}",
                display.width(),
                display.height()
            ))
            .arg("--socket")
            .arg(address.display())
            .stdin(Stdio::null())
            .process_group(0);
        if shell == Shell::Disabled {
            command.arg("--no-shell");
        }
        for variable in INHERITED_SESSION_VARIABLES {
            command.env_remove(variable);
        }
        if std::env::var_os("RUST_LOG").is_none() {
            command.env("RUST_LOG", DEFAULT_LOG_FILTER);
        }

        let mut child = command.spawn().map_err(Error::Spawn)?;
        let pid = Pid::from_child(&child);
        match pidfd_open(pid, PidfdFlags::empty()) {
            Ok(pidfd) => Ok(Compositor {
                child,
                pid,
                pidfd: Arc::new(pidfd),
            }),
            Err(error) => {
                child.kill().map_err(Error::Io)?;
                child.wait().map_err(Error::Io)?;
                Err(Error::Io(error.into()))
            }
        }
    }

    fn wait_until_ready(&self, address: &Address) -> Result<()> {
        let session_dir = address.session_dir();
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&session_dir)?;

        let mut watch = Watches::new().map_err(std::io::Error::other)?;
        watch
            .watch(&session_dir, Reach::Entries)
            .map_err(std::io::Error::other)?;

        let ready = address.environment_file();
        let deadline = Instant::now() + STARTUP_TIMEOUT;
        loop {
            if ready.exists() {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(Error::StartupTimeout(STARTUP_TIMEOUT));
            }
            if self.wait_for(&watch, remaining)? {
                tracing::warn!("headless compositor exited during startup");
                return Err(Error::Stopped);
            }
            watch.read().map_err(std::io::Error::other)?;
        }
    }

    fn wait_for(&self, watch: &Watches, timeout: Duration) -> Result<bool> {
        let timeout = timespec(timeout);
        let mut fds = [
            PollFd::new(&self.pidfd, PollFlags::IN),
            PollFd::new(watch, PollFlags::IN),
        ];
        match poll(&mut fds, Some(&timeout)) {
            Ok(_) | Err(rustix::io::Errno::INTR) => {}
            Err(error) => return Err(Error::Io(error.into())),
        }
        Ok(!fds[0].revents().is_empty())
    }

    fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    fn signal(&self, signal: Signal) {
        if let Err(error) = kill_process_group(self.pid, signal)
            && error != rustix::io::Errno::SRCH
        {
            tracing::warn!(%error, ?signal, "failed to signal compositor process group");
        }
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        self.signal(Signal::TERM);
        if !exited_within(&self.pidfd, SHUTDOWN_GRACE) {
            self.signal(Signal::KILL);
        }
        if let Err(error) = self.child.wait() {
            tracing::warn!(%error, "failed to reap compositor");
        }
    }
}

pub(crate) fn exited_within(pidfd: &OwnedFd, timeout: Duration) -> bool {
    let timeout = timespec(timeout);
    let mut fds = [PollFd::new(pidfd, PollFlags::IN)];
    matches!(poll(&mut fds, Some(&timeout)), Ok(1))
}

fn timespec(timeout: Duration) -> Timespec {
    Timespec::try_from(timeout).expect("timeout should fit in a timespec")
}

fn compositor_program() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| Some(exe.parent()?.join(COMPOSITOR)))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(COMPOSITOR))
}
