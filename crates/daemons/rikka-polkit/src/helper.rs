use rikka_sync::Mutex;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

use crate::error::{Error, Result};
use crate::protocol::Line;
use crate::secret::Secret;

const SOCKET: &str = "/run/polkit/agent-helper.socket";

const BINARY: &str = "/run/wrappers/bin/polkit-agent-helper-1";

pub(crate) struct Helper {
    reader: BufReader<Box<dyn Read + Send>>,
    writer: Box<dyn Write + Send>,
    interrupt: Stop,
}

impl Helper {
    pub(crate) fn start(user: &str, cookie: &str) -> Result<Helper> {
        let mut helper = match connect() {
            Some(socket) => {
                let mut helper = Helper::from_socket(socket)?;
                helper.line(user)?;
                helper
            }
            None => Helper::from_child(spawn(user)?)?,
        };
        helper.line(cookie)?;
        Ok(helper)
    }

    pub(crate) fn stop(&self) -> Stop {
        self.interrupt.clone()
    }

    pub(crate) fn read(&mut self) -> Result<Line> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => Err(Error::Broken("it closed the conversation".to_owned())),
            Ok(_) => Line::parse(&line)
                .ok_or_else(|| Error::Broken(format!("it said `{}`", line.trim_end()))),
            Err(error) => Err(Error::Broken(error.to_string())),
        }
    }

    pub(crate) fn respond(&mut self, secret: &Secret) -> Result<()> {
        self.line(secret.reveal())
    }

    fn from_socket(socket: UnixStream) -> Result<Helper> {
        let reading = socket.try_clone().map_err(broken)?;
        let writing = socket.try_clone().map_err(broken)?;
        Ok(Helper::new(reading, writing, Interrupt::Socket(socket)))
    }

    fn from_child(mut child: Child) -> Result<Helper> {
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(Error::Helperless(
                "it was started without the pipes to talk over".to_owned(),
            ));
        };
        Ok(Helper::new(
            stdout,
            stdin,
            Interrupt::Spawned(Mutex::new(Some(child))),
        ))
    }

    fn new(
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
        interrupt: Interrupt,
    ) -> Helper {
        Helper {
            reader: BufReader::new(Box::new(reader)),
            writer: Box::new(writer),
            interrupt: Stop(Arc::new(interrupt)),
        }
    }

    fn line(&mut self, text: &str) -> Result<()> {
        self.writer
            .write_all(text.as_bytes())
            .and_then(|()| self.writer.write_all(b"\n"))
            .and_then(|()| self.writer.flush())
            .map_err(broken)
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.interrupt.stop();
        self.interrupt.reap();
    }
}

impl std::fmt::Debug for Helper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Helper").finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Stop(Arc<Interrupt>);

#[derive(Debug)]
enum Interrupt {
    Socket(UnixStream),
    Spawned(Mutex<Option<Child>>),
}

impl Stop {
    pub(crate) fn stop(&self) {
        match &*self.0 {
            Interrupt::Socket(socket) => {
                let _ = socket.shutdown(Shutdown::Both);
            }
            Interrupt::Spawned(child) => {
                let child = child.lock();
                if let Some(child) = child.as_ref() {
                    unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) };
                }
            }
        }
    }

    fn reap(&self) {
        let Interrupt::Spawned(spawned) = &*self.0 else {
            return;
        };
        let helper_child = spawned.lock().take();
        if let Some(mut child) = helper_child {
            let _ = child.wait();
        }
    }
}

fn connect() -> Option<UnixStream> {
    if !Path::new(SOCKET).exists() {
        return None;
    }
    UnixStream::connect(SOCKET)
        .inspect_err(|error| {
            tracing::debug!("failed to connect to {SOCKET} ({error}), spawning helper");
        })
        .ok()
}

fn spawn(user: &str) -> Result<Child> {
    Command::new(BINARY)
        .arg(user)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|error| Error::Helperless(format!("{BINARY}: {error}")))
}

fn broken(error: std::io::Error) -> Error {
    Error::Broken(error.to_string())
}
