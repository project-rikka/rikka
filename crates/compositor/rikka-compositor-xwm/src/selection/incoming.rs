use std::os::fd::{AsFd, OwnedFd};
use std::time::Instant;

use x11rb_protocol::protocol::xproto::{AtomEnum, GetPropertyRequest, Window};

use crate::fault::{Outcome, describe};
use crate::pending::ExpectedReply;
use crate::poller::{Direction, Poller, SourceId};
use crate::selection::flow::{Incoming, Next};
use crate::selection::kind::SelectionKind;
use crate::selection::timeout::deadline;
use crate::wire::Reply;
use crate::xwm::Io;

#[derive(Debug)]
pub(crate) struct IncomingTransfer {
    pub(crate) kind: SelectionKind,
    pub(crate) window: Window,
    pub(crate) source: SourceId,
    pub(crate) deadline: Instant,
    fd: OwnedFd,
    flow: Incoming,
    converted: bool,
    completed: bool,
}

impl IncomingTransfer {
    pub(crate) fn new(
        kind: SelectionKind,
        window: Window,
        fd: OwnedFd,
        poller: &mut Poller,
    ) -> std::io::Result<Self> {
        let source = poller.source();
        poller.watch(fd.as_fd(), source, Direction::Write)?;
        Ok(Self {
            kind,
            window,
            source,
            deadline: deadline(),
            fd,
            flow: Incoming::default(),
            converted: false,
            completed: false,
        })
    }

    pub(crate) fn converted(&mut self, io: &mut Io<'_>, delivered: bool) -> Outcome<bool> {
        if !delivered {
            self.completed = true;
            return Ok(true);
        }
        self.converted = true;
        self.deadline = deadline();
        self.advance(io)
    }

    pub(crate) fn arrived(&mut self, io: &mut Io<'_>, reply: Reply) -> Outcome<bool> {
        let packet = match reply {
            Ok(packet) => packet,
            Err(error) => {
                tracing::debug!("failed to read X11 selection: {}", describe(&error));
                return Ok(true);
            }
        };
        self.deadline = deadline();
        self.flow.arrived(packet, io.link.atoms.INCR);
        self.advance(io)
    }

    pub(crate) fn property_arrived(&mut self, io: &mut Io<'_>) -> Outcome<bool> {
        if !self.converted {
            return Ok(false);
        }
        self.deadline = deadline();
        self.flow.announced();
        self.advance(io)
    }

    pub(crate) fn writable(&mut self, io: &mut Io<'_>) -> Outcome<bool> {
        if !self.converted {
            return Ok(false);
        }
        self.advance(io)
    }

    pub(crate) fn reusable(&self) -> bool {
        self.completed
    }

    pub(crate) fn release(&mut self, poller: &Poller) {
        poller.unwatch(self.fd.as_fd());
    }

    fn advance(&mut self, io: &mut Io<'_>) -> Outcome<bool> {
        loop {
            match self.flow.next() {
                Next::Write => match rustix::io::write(&self.fd, self.flow.pending()) {
                    Ok(length) => {
                        self.deadline = deadline();
                        self.flow.wrote(length);
                    }
                    Err(rustix::io::Errno::AGAIN) => return Ok(false),
                    Err(rustix::io::Errno::INTR) => {}
                    Err(error) => {
                        tracing::debug!("Wayland client closed X11 selection pipe: {error}");
                        return Ok(true);
                    }
                },
                Next::Fetch { offset } => {
                    let words = u32::try_from(io.link.transfer_chunk() / 4).unwrap_or(u32::MAX);
                    let sequence = io.link.wire.ask(GetPropertyRequest {
                        delete: true,
                        window: self.window,
                        property: io.link.atoms._WL_SELECTION,
                        type_: AtomEnum::ANY.into(),
                        long_offset: offset,
                        long_length: words,
                    })?;
                    io.pending.expect(
                        sequence,
                        ExpectedReply::Chunk {
                            source: self.source,
                        },
                    );
                    return Ok(false);
                }
                Next::Finish => {
                    self.completed = true;
                    return Ok(true);
                }
                Next::Wait => return Ok(false),
            }
        }
    }
}
