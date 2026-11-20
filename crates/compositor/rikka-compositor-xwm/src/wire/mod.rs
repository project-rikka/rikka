mod extensions;
mod reader;
mod writer;

use std::borrow::Cow;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use rustix::event::{PollFd, PollFlags, poll};
use rustix::io::Errno;
use rustix::time::Timespec;
use x11rb_protocol::connect::Connect;
use x11rb_protocol::connection::{Connection, PollReply, ReplyFdKind};
use x11rb_protocol::errors::ParseError;
use x11rb_protocol::id_allocator::IdAllocator;
use x11rb_protocol::protocol::Event;
use x11rb_protocol::protocol::xproto::{
    Atom, ChangePropertyRequest, GetInputFocusRequest, PropMode, Setup, Window,
};
use x11rb_protocol::x11_utils::{
    ExtensionInformation, ReplyRequest, Request, TryParse, VoidRequest, X11Error,
};
use x11rb_protocol::{DiscardMode, SequenceNumber};

use crate::error::{Error, Result};
use crate::fault::{Fault, Outcome};

use extensions::Extensions;
use reader::{Reader, Received};
use writer::{Writer, wire_length};

pub(crate) type Reply = std::result::Result<Vec<u8>, X11Error>;

const PATIENCE: Duration = Duration::from_secs(10);
pub(crate) const NONE: u32 = 0;
pub(crate) const CURRENT_TIME: u32 = 0;
pub(crate) const COPY_FROM_PARENT: u32 = 0;
pub(crate) const COPY_DEPTH_FROM_PARENT: u8 = 0;
const ERROR: u8 = 0;

#[derive(Debug)]
pub(crate) struct Wire {
    socket: UnixStream,
    protocol: Connection,
    ids: IdAllocator,
    extensions: Extensions,
    reader: Reader,
    writer: Writer,
    maximum_request_bytes: usize,
}

impl Wire {
    pub(crate) fn connect(socket: UnixStream) -> Result<(Self, Setup)> {
        let refused = |error: &dyn std::fmt::Display| Error::Handshake(error.to_string());
        socket
            .set_nonblocking(true)
            .map_err(|error| refused(&error))?;
        let (mut connect, request) = Connect::with_authorization(Vec::new(), Vec::new());
        let deadline = Instant::now() + PATIENCE;
        let mut sent = 0;
        while sent < request.len() {
            match rustix::io::write(&socket, &request[sent..]) {
                Ok(length) => sent += length,
                Err(Errno::AGAIN | Errno::INTR) => {
                    wait(&socket, PollFlags::OUT, deadline).map_err(|error| refused(&error))?;
                }
                Err(error) => return Err(refused(&error)),
            }
        }
        loop {
            match rustix::io::read(&socket, connect.buffer()) {
                Ok(0) => return Err(Error::Handshake("Xwayland hung up".into())),
                Ok(length) => {
                    if connect.advance(length) {
                        break;
                    }
                }
                Err(Errno::AGAIN | Errno::INTR) => {
                    wait(&socket, PollFlags::IN, deadline).map_err(|error| refused(&error))?;
                }
                Err(error) => return Err(refused(&error)),
            }
        }
        let setup = connect.into_setup().map_err(|error| refused(&error))?;
        let ids = IdAllocator::new(setup.resource_id_base, setup.resource_id_mask)
            .map_err(|error| refused(&error))?;
        let wire = Self {
            socket,
            protocol: Connection::new(),
            ids,
            extensions: Extensions::default(),
            reader: Reader::default(),
            writer: Writer::default(),
            maximum_request_bytes: usize::from(setup.maximum_request_length) * 4,
        };
        Ok((wire, setup))
    }

    pub(crate) fn learn_extension(
        &mut self,
        name: &'static str,
        information: ExtensionInformation,
    ) {
        self.extensions.insert(name, information);
    }

    pub(crate) fn has_extension(&self, name: &str) -> bool {
        self.extensions.opcode(name).is_some()
    }

    pub(crate) fn allow_request_words(&mut self, words: u32) {
        let bytes = usize::try_from(words)
            .unwrap_or(usize::MAX)
            .saturating_mul(4);
        self.maximum_request_bytes = self.maximum_request_bytes.max(bytes);
    }

    pub(crate) fn maximum_request_bytes(&self) -> usize {
        self.maximum_request_bytes
    }

    pub(crate) fn send<R: VoidRequest>(&mut self, request: R) -> Outcome<SequenceNumber> {
        let opcode = self.opcode(R::EXTENSION_NAME)?;
        let (bytes, _) = request.serialize(opcode);
        self.emit(&[&bytes], ReplyFdKind::NoReply)
    }

    pub(crate) fn ask<R: ReplyRequest>(&mut self, request: R) -> Outcome<SequenceNumber> {
        let opcode = self.opcode(R::EXTENSION_NAME)?;
        let (bytes, _) = request.serialize(opcode);
        self.emit(&[&bytes], ReplyFdKind::ReplyWithoutFDs)
    }

    pub(crate) fn refusal<R: VoidRequest>(&mut self, request: R) -> Outcome<Option<X11Error>> {
        let opcode = self.opcode(R::EXTENSION_NAME)?;
        let (bytes, _) = request.serialize(opcode);
        let sequence = self.emit_checked(&[&bytes], ReplyFdKind::NoReply)?;
        let sync = self.ask(GetInputFocusRequest)?;
        self.wait(sync)?.map_err(Fault::Refused)?;
        match self.protocol.poll_check_for_reply_or_error(sequence) {
            PollReply::Reply(packet) => Ok(Some(X11Error::try_parse(&packet, &self.extensions)?)),
            PollReply::NoReply | PollReply::TryAgain => Ok(None),
        }
    }

    pub(crate) fn change_property(
        &mut self,
        window: Window,
        property: Atom,
        type_: Atom,
        format: u8,
        data: &[u8],
    ) -> Outcome {
        let elements = u32::try_from(data.len() / usize::from(format / 8))
            .map_err(|_| Fault::Oversized(data.len()))?;
        let (pieces, _) = ChangePropertyRequest {
            mode: PropMode::REPLACE,
            window,
            property,
            type_,
            format,
            data_len: elements,
            data: Cow::Borrowed(data),
        }
        .serialize();
        let [header, data, padding] = &pieces;
        self.emit(&[header, data, padding], ReplyFdKind::NoReply)?;
        Ok(())
    }

    pub(crate) fn generate_id(&mut self) -> Outcome<u32> {
        self.ids.generate_id().ok_or(Fault::Exhausted)
    }

    pub(crate) fn receive(&mut self) -> Outcome<bool> {
        let protocol = &mut self.protocol;
        match self
            .reader
            .receive(&self.socket, |packet| protocol.enqueue_packet(packet))?
        {
            Received::Nothing => Ok(false),
            Received::Some => Ok(true),
            Received::Closed => Err(Fault::closed()),
        }
    }

    pub(crate) fn reply(&mut self, sequence: SequenceNumber) -> Outcome<Option<Reply>> {
        let Some((packet, _)) = self.protocol.poll_for_reply_or_error(sequence) else {
            return Ok(None);
        };
        if packet[0] == ERROR {
            return Ok(Some(Err(X11Error::try_parse(&packet, &self.extensions)?)));
        }
        Ok(Some(Ok(packet)))
    }

    pub(crate) fn event(&mut self) -> Option<(Vec<u8>, SequenceNumber)> {
        self.protocol.poll_for_event_with_sequence()
    }

    pub(crate) fn parse_event(&self, raw: &[u8]) -> std::result::Result<Event, ParseError> {
        Event::parse(raw, &self.extensions)
    }

    pub(crate) fn flush(&mut self) -> Outcome<bool> {
        Ok(self.writer.flush(&self.socket)?)
    }

    pub(crate) fn wait(&mut self, sequence: SequenceNumber) -> Outcome<Reply> {
        let deadline = Instant::now() + PATIENCE;
        loop {
            if let Some(reply) = self.reply(sequence)? {
                return Ok(reply);
            }
            let blocked = self.flush()?;
            let flags = if blocked {
                PollFlags::IN | PollFlags::OUT
            } else {
                PollFlags::IN
            };
            wait(&self.socket, flags, deadline)?;
            self.receive()?;
        }
    }

    fn opcode(&self, extension: Option<&'static str>) -> Outcome<u8> {
        match extension {
            None => Ok(0),
            Some(name) => self.extensions.opcode(name).ok_or(Fault::Missing(name)),
        }
    }

    fn emit(&mut self, pieces: &[&[u8]], kind: ReplyFdKind) -> Outcome<SequenceNumber> {
        let sequence = self.emit_checked(pieces, kind)?;
        if kind == ReplyFdKind::NoReply {
            self.protocol
                .discard_reply(sequence, DiscardMode::DiscardReply);
        }
        Ok(sequence)
    }

    fn emit_checked(&mut self, pieces: &[&[u8]], kind: ReplyFdKind) -> Outcome<SequenceNumber> {
        let length: usize = pieces.iter().map(|piece| piece.len()).sum();
        if wire_length(length) > self.maximum_request_bytes {
            return Err(Fault::Oversized(length));
        }
        let sequence = loop {
            match self.protocol.send_request(kind) {
                Some(sequence) => break sequence,
                None => self.sync()?,
            }
        };
        self.writer.push(pieces, length);
        Ok(sequence)
    }

    fn sync(&mut self) -> Outcome {
        let (bytes, _) = Request::serialize(GetInputFocusRequest, 0);
        let sequence = self
            .protocol
            .send_request(ReplyFdKind::ReplyWithoutFDs)
            .ok_or(Fault::Exhausted)?;
        self.writer.push(&[&bytes], bytes.len());
        self.protocol
            .discard_reply(sequence, DiscardMode::DiscardReplyAndError);
        Ok(())
    }
}

impl AsFd for Wire {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.socket.as_fd()
    }
}

pub(crate) fn parse<R: TryParse>(packet: &[u8]) -> Outcome<R> {
    Ok(R::try_parse(packet)?.0)
}

fn wait(socket: &UnixStream, flags: PollFlags, deadline: Instant) -> std::io::Result<()> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(std::io::ErrorKind::TimedOut.into());
    }
    let timeout = Timespec {
        tv_sec: i64::try_from(left.as_secs()).unwrap_or(i64::MAX),
        tv_nsec: i64::from(left.subsec_nanos()),
    };
    let mut fds = [PollFd::new(socket, flags)];
    match poll(&mut fds, Some(&timeout)) {
        Ok(_) | Err(Errno::INTR) => Ok(()),
        Err(error) => Err(error.into()),
    }
}
