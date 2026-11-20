use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use jiff::Timestamp;

use crate::logging::{Journal, open_shell_log};

#[derive(Debug)]
pub(super) struct ShellLog {
    journal: Journal,
    socket: String,
    sink: Sink,
    mirrored: bool,
}

#[derive(Debug)]
enum Sink {
    Unopened,
    Open { path: PathBuf, file: File },
    Unavailable,
}

impl ShellLog {
    pub(super) fn new(journal: Journal, socket: &str) -> ShellLog {
        ShellLog {
            journal,
            socket: socket.to_owned(),
            sink: Sink::Unopened,
            mirrored: true,
        }
    }

    pub(super) fn path(&self) -> Option<&Path> {
        match &self.sink {
            Sink::Open { path, .. } => Some(path),
            Sink::Unopened | Sink::Unavailable => None,
        }
    }

    pub(super) fn begin(&mut self, program: &Path, pid: u32) {
        if matches!(self.sink, Sink::Unopened) {
            self.sink = match open_shell_log(self.journal, &self.socket) {
                Ok((path, file)) => Sink::Open { path, file },
                Err(error) => {
                    tracing::warn!("failed to open desktop log: {error}");
                    Sink::Unavailable
                }
            };
        }
        let header = format!(
            "--- {} started as pid {pid} at {} ---\n",
            program.display(),
            Timestamp::now()
        );
        self.write_file(header.as_bytes());
    }

    pub(super) fn write(&mut self, bytes: &[u8]) {
        self.write_file(bytes);
        if self.mirrored
            && let Err(error) = io::stderr().write_all(bytes)
        {
            self.mirrored = false;
            tracing::warn!("failed to mirror desktop output to stderr: {error}");
        }
    }

    fn write_file(&mut self, bytes: &[u8]) {
        let Sink::Open { path, file } = &mut self.sink else {
            return;
        };
        if let Err(error) = file.write_all(bytes) {
            tracing::warn!("failed to write desktop log {}: {error}", path.display());
            self.sink = Sink::Unavailable;
        }
    }
}
