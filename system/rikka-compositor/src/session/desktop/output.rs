use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::process::{Command, Stdio};

use rikka_compositor_reactor::Token;
use rustix::fs::{OFlags, fcntl_setfl};
use rustix::io::Errno;
use rustix::pipe::{PipeFlags, pipe_with};

use super::shell_log::ShellLog;
use crate::crash::Tail;

const CHUNK: usize = 16 * 1024;
const CHUNKS_PER_DRAIN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Flow {
    Open,
    Closed,
}

#[derive(Debug)]
pub(super) struct Output {
    reader: OwnedFd,
    watch: Option<Token>,
    tail: Tail,
}

impl Output {
    pub(super) fn attach(command: &mut Command) -> io::Result<Output> {
        let (reader, writer) = pipe_with(PipeFlags::CLOEXEC)?;
        fcntl_setfl(&reader, OFlags::NONBLOCK)?;
        command
            .stdout(Stdio::from(writer.try_clone()?))
            .stderr(Stdio::from(writer));
        Ok(Output {
            reader,
            watch: None,
            tail: Tail::default(),
        })
    }

    pub(super) fn reader(&self) -> BorrowedFd<'_> {
        self.reader.as_fd()
    }

    pub(super) fn watched(&mut self, watch: Token) {
        self.watch = Some(watch);
    }

    pub(super) fn take_watch(&mut self) -> Option<Token> {
        self.watch.take()
    }

    pub(super) fn tail(&self) -> &Tail {
        &self.tail
    }

    pub(super) fn drain(&mut self, log: &mut ShellLog) -> Flow {
        let mut chunk = [0; CHUNK];
        let mut chunks = 0;
        while chunks < CHUNKS_PER_DRAIN {
            match rustix::io::read(&self.reader, &mut chunk) {
                Ok(0) => return Flow::Closed,
                Ok(read) => {
                    chunks += 1;
                    self.tail.push(&chunk[..read]);
                    log.write(&chunk[..read]);
                }
                Err(Errno::INTR) => {}
                Err(Errno::AGAIN) => return Flow::Open,
                Err(error) => {
                    tracing::warn!("failed to read desktop output: {error}");
                    return Flow::Closed;
                }
            }
        }
        Flow::Open
    }
}
