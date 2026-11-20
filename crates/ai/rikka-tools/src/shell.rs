use rikka_sync::Mutex;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use rikka_prompt::Prompt;
use rikka_types::CancelToken;
use rustix::io::Errno;
use rustix::process::{Pid, Signal, kill_process_group};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::Notify;

use crate::error::Result;
use crate::limits::most;
use crate::logs::Log;
use crate::machine::Machine;
use crate::output_buffer::OutputBuffer;

const FLUSHING: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    timeout_ms: u64,
    longest: u64,
    output: usize,
    retain: usize,
}

impl Limits {
    pub(crate) fn read(prompt: &Prompt) -> Result<Limits> {
        Ok(Limits {
            timeout_ms: prompt.limit("timeout_ms")?,
            longest: prompt.limit("max_timeout_ms")?,
            output: most(prompt, "output_bytes")?,
            retain: most(prompt, "retain_bytes")?,
        })
    }

    pub(crate) fn output(self) -> usize {
        self.output
    }

    pub(crate) fn timeout(self, requested: Option<u64>) -> Duration {
        Duration::from_millis(requested.unwrap_or(self.timeout_ms).min(self.longest))
    }

    pub(crate) fn retain(self) -> usize {
        self.retain
    }
}

pub(crate) struct CommandSpec<'a> {
    pub(crate) command: &'a str,
    pub(crate) at: &'a Path,
    pub(crate) machine: Option<&'a dyn Machine>,
    pub(crate) log: Option<Log>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ended {
    Exited(i32),
    Signalled,
}

#[derive(Clone, Debug)]
pub(crate) struct Running {
    trail: Arc<Mutex<OutputBuffer>>,
    ended: Arc<Mutex<Option<Ended>>>,
    exited: Arc<Notify>,
    killing: CancelToken,
}

impl Running {
    pub(crate) fn start(
        command: CommandSpec<'_>,
        retain: usize,
    ) -> std::result::Result<Running, String> {
        let mut shell = Command::new("bash");
        shell
            .arg("-lc")
            .arg(command.command)
            .current_dir(command.at)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("PAGER", "cat")
            .env("GIT_PAGER", "cat")
            .env("TERM", "dumb")
            .process_group(0);
        if let Some(machine) = command.machine {
            machine.enter(shell.as_std_mut());
        }
        let mut child = shell
            .spawn()
            .map_err(|error| format!("bash could not be started: {error}"))?;

        let running = Running {
            trail: Arc::new(Mutex::new(OutputBuffer::new(retain))),
            ended: Arc::new(Mutex::new(None)),
            exited: Arc::new(Notify::new()),
            killing: CancelToken::new(),
        };

        let mut out = child.stdout.take().expect("stdout should be piped");
        let mut err = child.stderr.take().expect("stderr should be piped");
        let reading = Arc::clone(&running.trail);
        let log = command.log;
        let mut draining = tokio::spawn(async move {
            tokio::join!(
                drain(&mut out, &reading, log.as_ref()),
                drain(&mut err, &reading, log.as_ref())
            );
        });

        let watching = running.clone();
        tokio::spawn(async move {
            let leader = child
                .id()
                .and_then(|leader| i32::try_from(leader).ok())
                .and_then(Pid::from_raw);
            let killing = watching.killing.clone();

            let ended = tokio::select! {
                () = killing.cancelled() => {
                    if let Some(leader) = leader
                        && let Err(error) = kill_process_group(leader, Signal::KILL)
                        && error != Errno::SRCH
                    {
                        tracing::warn!("failed to kill a command: {error}");
                    }
                    child.wait().await
                }
                ended = child.wait() => ended,
            };

            if tokio::time::timeout(FLUSHING, &mut draining).await.is_err() {
                draining.abort();
            }

            let ended = match ended.ok().and_then(|status| status.code()) {
                Some(code) => Ended::Exited(code),
                None => Ended::Signalled,
            };
            *watching.ended.lock() = Some(ended);
            watching.exited.notify_waiters();
        });

        Ok(running)
    }

    pub(crate) fn trail(&self) -> &Arc<Mutex<OutputBuffer>> {
        &self.trail
    }

    pub(crate) fn ended(&self) -> Option<Ended> {
        *self.ended.lock()
    }

    pub(crate) fn kill(&self) {
        self.killing.cancel();
    }

    pub(crate) async fn wait_exit(&self) -> Ended {
        loop {
            let waiting = self.exited.notified();
            tokio::pin!(waiting);
            waiting.as_mut().enable();

            if let Some(ended) = self.ended() {
                return ended;
            }
            waiting.await;
        }
    }

    pub(crate) async fn wait_timeout(&self, timeout_after: Duration) -> Option<Ended> {
        tokio::time::timeout(timeout_after, self.wait_exit())
            .await
            .ok()
    }

    pub(crate) fn output(&self, room: usize) -> String {
        self.trail.lock().summary(room).trim_end().to_owned()
    }

    pub(crate) fn read(&self, room: usize) -> String {
        self.trail.lock().read_new(room).trim_end().to_owned()
    }
}

pub(crate) struct KillOnDrop(Option<Running>);

impl KillOnDrop {
    pub(crate) fn new(running: &Running) -> KillOnDrop {
        KillOnDrop(Some(running.clone()))
    }

    pub(crate) fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if let Some(running) = self.0.take() {
            running.kill();
        }
    }
}

async fn drain(
    pipe: &mut (impl AsyncRead + Unpin),
    trail: &Mutex<OutputBuffer>,
    log: Option<&Log>,
) {
    let mut buffer = [0u8; 8192];

    loop {
        match pipe.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(read) => {
                trail.lock().push(&buffer[..read]);
                if let Some(log) = log {
                    log.wrote(&buffer[..read]);
                }
            }
        }
    }
}
