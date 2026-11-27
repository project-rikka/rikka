use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::io::Errno;
use rustix::net::sockopt::socket_peercred;
use rustix::pipe::{PipeFlags, pipe_with};
use rustix::process::getuid;
use tempfile::TempDir;

use crate::channel::frame;
use crate::channel::message::Incoming;

const SOCKET: &str = "channel";

pub(crate) trait Inbound: Send + Sync + 'static {
    fn connected(&self, stream: UnixStream);
    fn received(&self, message: Incoming);
    fn disconnected(&self);
    fn overdue(&self);
}

#[derive(Debug)]
pub(crate) struct Rendezvous {
    folder: TempDir,
}

impl Rendezvous {
    pub(crate) fn path(&self) -> PathBuf {
        self.folder.path().join(SOCKET)
    }
}

pub(crate) struct Listening {
    stop: OwnedFd,
    pub(crate) thread: JoinHandle<()>,
}

impl Listening {
    pub(crate) fn stop(self) -> JoinHandle<()> {
        drop(self.stop);
        self.thread
    }
}

impl std::fmt::Debug for Listening {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Listening").finish_non_exhaustive()
    }
}

pub(crate) fn open(runtime_dir: &Path) -> io::Result<(Rendezvous, UnixListener)> {
    let folder = tempfile::Builder::new()
        .prefix("rikka-web-")
        .tempdir_in(runtime_dir)?;
    let path = folder.path().join(SOCKET);
    let listener = UnixListener::bind(&path)?;
    Ok((Rendezvous { folder }, listener))
}

pub(crate) fn serve(
    listener: UnixListener,
    patience: Duration,
    inbound: Arc<dyn Inbound>,
) -> io::Result<Listening> {
    let (stopped, stop) = pipe_with(PipeFlags::CLOEXEC)?;
    let thread = std::thread::Builder::new()
        .name("rikka-web-channel".into())
        .spawn(move || run(&listener, &stopped, patience, inbound.as_ref()))?;
    Ok(Listening { stop, thread })
}

enum Wait {
    Ready,
    Stopped,
    TimedOut,
    Failed,
}

fn wait(source: impl AsFd, stopped: &OwnedFd, timeout: Option<Duration>) -> Wait {
    let timeout = timeout.map(|timeout| Timespec::try_from(timeout).expect("patience fits"));
    let mut watched = [
        PollFd::new(&source, PollFlags::IN),
        PollFd::new(stopped, PollFlags::IN),
    ];
    loop {
        match poll(&mut watched, timeout.as_ref()) {
            Err(Errno::INTR) => {}
            Err(error) => {
                tracing::warn!(%error, "failed to poll the extension channel");
                return Wait::Failed;
            }
            Ok(0) => return Wait::TimedOut,
            Ok(_) if !watched[1].revents().is_empty() => return Wait::Stopped,
            Ok(_) => return Wait::Ready,
        }
    }
}

fn run(listener: &UnixListener, stopped: &OwnedFd, patience: Duration, inbound: &dyn Inbound) {
    let mut first = Some(patience);
    loop {
        match wait(listener, stopped, first) {
            Wait::Ready => {}
            Wait::TimedOut => {
                first = None;
                inbound.overdue();
                continue;
            }
            Wait::Stopped | Wait::Failed => return,
        }
        let stream = match listener.accept() {
            Ok((stream, _)) => stream,
            Err(error) => {
                tracing::warn!(%error, "failed to accept the extension channel");
                continue;
            }
        };
        if !trusted(&stream) {
            continue;
        }
        first = None;
        if !converse(stream, stopped, inbound) {
            return;
        }
    }
}

fn trusted(stream: &UnixStream) -> bool {
    match socket_peercred(stream) {
        Ok(credentials) if credentials.uid == getuid() => true,
        Ok(credentials) => {
            tracing::warn!(
                uid = credentials.uid.as_raw(),
                "refused a foreign extension channel"
            );
            false
        }
        Err(error) => {
            tracing::warn!(%error, "failed to identify the extension channel");
            false
        }
    }
}

fn converse(mut stream: UnixStream, stopped: &OwnedFd, inbound: &dyn Inbound) -> bool {
    match stream.try_clone() {
        Ok(writer) => inbound.connected(writer),
        Err(error) => {
            tracing::warn!(%error, "failed to share the extension channel");
            return true;
        }
    }
    let running = loop {
        match wait(&stream, stopped, None) {
            Wait::Ready => {}
            Wait::Stopped => break false,
            Wait::TimedOut | Wait::Failed => break true,
        }
        match frame::read(&mut stream) {
            Ok(Some(body)) => match serde_json::from_slice::<Incoming>(&body) {
                Ok(message) => inbound.received(message),
                Err(error) => tracing::warn!(%error, "malformed message from the Rikka extension"),
            },
            Ok(None) => break true,
            Err(error) => {
                tracing::warn!(%error, "failed to read from the Rikka extension");
                break true;
            }
        }
    };
    inbound.disconnected();
    running
}
