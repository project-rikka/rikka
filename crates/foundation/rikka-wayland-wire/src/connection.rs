use std::cell::{Ref, RefCell};
use std::fmt;
use std::io::{self, IoSliceMut};
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, OwnedFd};

use rustix::io::Errno;
use rustix::net::{RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, recvmsg};

use crate::error::{Result, WireError};
use crate::ids::{MAX_FDS_PER_MESSAGE, MAX_MESSAGE_SIZE};
use crate::incoming::Incoming;
use crate::message::{Header, Message};
use crate::outgoing::{Flush, Outgoing};
use crate::writer::Writer;

pub const INCOMING_INITIAL: usize = 4 << 10;
pub const INCOMING_LIMIT: usize = 1 << 20;
const QUEUED_FDS_LIMIT: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Read(usize),
    WouldBlock,
    Full,
    Closed,
}

pub struct Connection {
    socket: RefCell<Option<OwnedFd>>,
    incoming: RefCell<Incoming>,
    outgoing: RefCell<Outgoing>,
}

impl Connection {
    pub fn new(socket: impl Into<OwnedFd>) -> Self {
        Self {
            socket: RefCell::new(Some(socket.into())),
            incoming: RefCell::new(Incoming::new(INCOMING_INITIAL, INCOMING_LIMIT)),
            outgoing: RefCell::new(Outgoing::new()),
        }
    }

    pub fn socket(&self) -> Option<Ref<'_, OwnedFd>> {
        Ref::filter_map(self.socket.borrow(), Option::as_ref).ok()
    }

    pub fn incoming_capacity(&self) -> usize {
        self.incoming.borrow().capacity()
    }

    pub fn buffered(&self) -> usize {
        self.incoming.borrow().len()
    }

    pub fn queued_fds(&self) -> usize {
        self.incoming.borrow().fds.len()
    }

    pub fn pending_output(&self) -> usize {
        self.outgoing.borrow().pending()
    }

    pub fn fill(&self) -> Result<Fill> {
        let socket = self.socket.borrow();
        let Some(socket) = socket.as_ref() else {
            return Ok(Fill::Closed);
        };
        let mut incoming = self.incoming.borrow_mut();
        let mut space =
            [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS_PER_MESSAGE))];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let received = loop {
            let Some(spare) = incoming.spare() else {
                return Ok(Fill::Full);
            };
            let mut chunks = [IoSliceMut::new(spare)];
            match recvmsg(
                socket,
                &mut chunks,
                &mut control,
                RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT,
            ) {
                Ok(received) => break received,
                Err(Errno::INTR) => {}
                Err(Errno::AGAIN) => return Ok(Fill::WouldBlock),
                Err(error) => return Err(WireError::Io(error.into())),
            }
        };
        let queued = incoming.fds.len();
        for message in control.drain() {
            if let RecvAncillaryMessage::ScmRights(fds) = message {
                incoming.fds.extend(fds);
            }
        }
        let arrived = incoming.fds.len() - queued;
        if received.flags.contains(ReturnFlags::CTRUNC)
            || arrived > MAX_FDS_PER_MESSAGE
            || incoming.fds.len() > QUEUED_FDS_LIMIT
        {
            return Err(WireError::TooManyFds);
        }
        if received.bytes == 0 {
            return Ok(Fill::Closed);
        }
        incoming.commit(received.bytes);
        Ok(Fill::Read(received.bytes))
    }

    pub fn peek(&self) -> Result<Option<Header>> {
        let incoming = self.incoming.borrow();
        let Some([object, word]) = incoming.header() else {
            return Ok(None);
        };
        let size = (word >> 16) as usize;
        if !(8..=MAX_MESSAGE_SIZE).contains(&size) || !size.is_multiple_of(4) {
            return Err(WireError::BadSize(size));
        }
        if incoming.len() < size {
            return Ok(None);
        }
        Ok(Some(Header {
            object,
            opcode: (word & 0xFFFF) as u16,
            size,
        }))
    }

    pub fn message(&self, header: Header) -> Message<'_> {
        Message::new(header, self.incoming.borrow_mut())
    }

    pub fn next_message(&self) -> Result<Option<Message<'_>>> {
        Ok(self.peek()?.map(|header| self.message(header)))
    }

    pub fn write(&self, object: u32, opcode: u16) -> Writer<'_> {
        Writer::new(self.outgoing.borrow_mut(), object, opcode)
    }

    pub fn flush(&self) -> Result<Flush> {
        match self.socket.borrow().as_ref() {
            Some(socket) => self.outgoing.borrow_mut().flush(socket.as_fd()),
            None => Err(WireError::Io(io::ErrorKind::NotConnected.into())),
        }
    }

    pub fn close(&self) {
        self.socket.borrow_mut().take();
        self.outgoing.borrow_mut().clear();
        self.incoming.borrow_mut().clear();
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("Connection");
        if let Ok(socket) = self.socket.try_borrow() {
            debug.field("socket", &*socket);
        }
        if let Ok(incoming) = self.incoming.try_borrow() {
            debug
                .field("buffered", &incoming.len())
                .field("queued_fds", &incoming.fds.len());
        }
        if let Ok(outgoing) = self.outgoing.try_borrow() {
            debug.field("pending_output", &outgoing.pending());
        }
        debug.finish_non_exhaustive()
    }
}
