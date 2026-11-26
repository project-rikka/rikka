use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::log::{LevelTracker, LogLine};
use crate::stop;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Exit {
    pub code: Option<i32>,
    pub played: Duration,
}

impl Exit {
    #[must_use]
    pub fn crashed(&self) -> bool {
        self.code.is_some_and(|code| code != 0)
    }
}

#[derive(Debug)]
pub struct Game {
    pid: u32,
    exited: Arc<AtomicBool>,
}

type Lines = Arc<dyn Fn(LogLine) + Send + Sync>;

fn reader(
    name: &str,
    stream: impl Read + Send + 'static,
    lines: Lines,
) -> std::io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || {
            let mut tracker = LevelTracker::default();
            let mut reader = BufReader::new(stream);
            let mut buffer = Vec::new();
            loop {
                buffer.clear();
                match reader.read_until(b'\n', &mut buffer) {
                    Ok(0) => return,
                    Ok(_) => {
                        let text = String::from_utf8_lossy(&buffer)
                            .trim_end_matches(['\r', '\n'])
                            .to_owned();
                        lines(tracker.line(text));
                    }
                    Err(error) => {
                        tracing::debug!("game output closed: {error}");
                        return;
                    }
                }
            }
        })
}

#[cfg(windows)]
fn detach(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    command.creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0);
}

#[cfg(unix)]
fn detach(command: &mut Command) {
    use std::os::unix::process::CommandExt as _;
    command.process_group(0);
}

impl Game {
    pub fn spawn(
        mut command: Command,
        on_line: impl Fn(LogLine) + Send + Sync + 'static,
        on_exit: impl FnOnce(Exit) + Send + 'static,
    ) -> Result<Game> {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        detach(&mut command);
        let started = Instant::now();
        let mut child = command.spawn().map_err(Error::Spawn)?;
        let pid = child.id();
        let lines: Lines = Arc::new(on_line);
        let mut readers = Vec::with_capacity(2);
        if let Some(stdout) = child.stdout.take() {
            readers.push(
                reader("rikka-launcher-out", stdout, Arc::clone(&lines)).map_err(Error::Spawn)?,
            );
        }
        if let Some(stderr) = child.stderr.take() {
            readers.push(reader("rikka-launcher-err", stderr, lines).map_err(Error::Spawn)?);
        }
        let exited = Arc::new(AtomicBool::new(false));
        let marked = Arc::clone(&exited);
        std::thread::Builder::new()
            .name("rikka-launcher-game".to_owned())
            .spawn(move || {
                let status = child.wait();
                marked.store(true, Ordering::Release);
                for reader in readers {
                    if reader.join().is_err() {
                        tracing::warn!("game output reader panicked");
                    }
                }
                let code = match status {
                    Ok(status) => status.code(),
                    Err(error) => {
                        tracing::warn!("failed to wait for the game: {error}");
                        None
                    }
                };
                on_exit(Exit {
                    code,
                    played: started.elapsed(),
                });
            })
            .map_err(Error::Spawn)?;
        Ok(Game { pid, exited })
    }

    #[must_use]
    pub fn is_running(&self) -> bool {
        !self.exited.load(Ordering::Acquire)
    }

    pub fn stop(&self) -> Result<()> {
        if !self.is_running() {
            return Ok(());
        }
        stop::terminate(self.pid).map_err(Error::Stop)
    }
}
