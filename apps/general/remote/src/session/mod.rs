mod clipboard;
mod network;
mod sound;
mod video;

use std::path::PathBuf;
use std::sync::Arc;

use rikka_remote_link::Pin;
use rikka_remote_wire::{
    CursorShape, Display, EndReason, InputEvent, Position, Refusal, Stats, Stream, Viewport,
};
use rikka_ui::Feed;
use rikka_worker::{Stop, Worker};
use tokio::sync::{Notify, mpsc};
use zeroize::Zeroizing;

const THREAD: &str = "rikka-remote-viewer-link";

#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub(crate) address: String,
    pub(crate) pin: Option<Pin>,
    pub(crate) token: Option<Vec<u8>>,
    pub(crate) device: String,
}

#[derive(Debug)]
pub(crate) enum Command {
    SignIn {
        password: Zeroizing<String>,
        remember: bool,
    },
    Viewport(Viewport),
    Input(InputEvent),
    Motion(Position),
    PointerLock(bool),
    Keyframe,
    Upload(PathBuf),
    SignOut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Refused {
    pub(crate) reason: Refusal,
    pub(crate) retry_after_ms: u32,
    pub(crate) detail: String,
}

#[derive(Debug)]
pub(crate) enum Report {
    Connected {
        host: String,
        user: String,
        pin: Option<Pin>,
    },
    Password(Option<Refused>),
    TokenRefused,
    SignedIn {
        token: Option<Vec<u8>>,
    },
    Display(Display),
    Stream(Stream),
    CursorShape(CursorShape),
    Cursor(u32),
    Pointer {
        x: i32,
        y: i32,
    },
    Stats(Stats),
    FileSaved(String),
    FileFailed(String),
    PinChanged {
        seen: Pin,
    },
    Ended(EndReason),
    Failed(String),
}

struct Leave(Arc<Notify>);

impl Stop for Leave {
    fn stop(&self) {
        self.0.notify_one();
    }
}

pub(crate) struct Link {
    commands: mpsc::UnboundedSender<Command>,
    _worker: Worker<Leave>,
}

impl Link {
    pub(crate) fn start(
        target: Target,
        feed: Feed,
        reports: impl Fn(Report) + Send + Sync + 'static,
    ) -> Result<Link, rikka_worker::Error<std::io::Error>> {
        let (commands, received) = mpsc::unbounded_channel();
        let leave = Arc::new(Notify::new());
        let left = Arc::clone(&leave);
        let worker = Worker::spawn(THREAD, Leave(leave), move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            Ok(move || network::run(runtime, target, feed, received, left, reports))
        })?;
        Ok(Link {
            commands,
            _worker: worker,
        })
    }

    pub(crate) fn commands(&self) -> mpsc::UnboundedSender<Command> {
        self.commands.clone()
    }

    pub(crate) fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            tracing::debug!("the connection already ended");
        }
    }
}

impl std::fmt::Debug for Link {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Link").finish_non_exhaustive()
    }
}
