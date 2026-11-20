use std::collections::VecDeque;
use std::io::IoSlice;
use std::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rustix::io::Errno;
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};

use crate::error::{Result, WireError};
use crate::ids::MAX_FDS_PER_MESSAGE;

pub const OUTGOING_LIMIT: usize = 4 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flush {
    Done,
    Pending,
}

#[derive(Debug)]
pub(crate) struct Outgoing {
    bytes: Vec<u8>,
    sent: usize,
    origin: u64,
    fds: VecDeque<(u64, OwnedFd)>,
}

impl Outgoing {
    pub(crate) fn new() -> Self {
        Self {
            bytes: Vec::new(),
            sent: 0,
            origin: 0,
            fds: VecDeque::new(),
        }
    }

    pub(crate) fn pending(&self) -> usize {
        self.bytes.len() - self.sent
    }

    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn fd_count(&self) -> usize {
        self.fds.len()
    }

    pub(crate) fn extend(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub(crate) fn patch(&mut self, at: usize, word: u32) {
        self.bytes[at..at + 4].copy_from_slice(&word.to_ne_bytes());
    }

    pub(crate) fn attach(&mut self, message_start: usize, fd: OwnedFd) {
        self.fds.push_back((self.origin + message_start as u64, fd));
    }

    pub(crate) fn rollback(&mut self, length: usize, fd_count: usize) {
        self.bytes.truncate(length);
        self.fds.truncate(fd_count);
    }

    pub(crate) fn clear(&mut self) {
        self.origin += self.bytes.len() as u64;
        self.bytes.clear();
        self.fds.clear();
        self.sent = 0;
    }

    pub(crate) fn flush(&mut self, socket: BorrowedFd<'_>) -> Result<Flush> {
        let result = self.send_all(socket);
        self.compact();
        result
    }

    fn send_all(&mut self, socket: BorrowedFd<'_>) -> Result<Flush> {
        while self.pending() > 0 {
            let batch = self.fds.len().min(MAX_FDS_PER_MESSAGE);
            let end = match self.fds.get(batch) {
                Some((position, _)) => (position - self.origin) as usize,
                None => self.bytes.len(),
            };
            match self.send_batch(socket, batch, end) {
                Ok(length) => {
                    self.sent += length;
                    self.fds.drain(..batch);
                }
                Err(Errno::INTR) => {}
                Err(Errno::AGAIN) => return Ok(Flush::Pending),
                Err(error) => return Err(WireError::Io(error.into())),
            }
        }
        Ok(Flush::Done)
    }

    fn send_batch(
        &self,
        socket: BorrowedFd<'_>,
        batch: usize,
        end: usize,
    ) -> std::result::Result<usize, Errno> {
        let mut borrowed = [socket; MAX_FDS_PER_MESSAGE];
        for (slot, (_, fd)) in borrowed.iter_mut().zip(self.fds.iter().take(batch)) {
            *slot = fd.as_fd();
        }
        let mut space =
            [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS_PER_MESSAGE))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if batch > 0 {
            control.push(SendAncillaryMessage::ScmRights(&borrowed[..batch]));
        }
        let chunk = [IoSlice::new(&self.bytes[self.sent..end])];
        sendmsg(
            socket,
            &chunk,
            &mut control,
            SendFlags::NOSIGNAL | SendFlags::DONTWAIT,
        )
    }

    fn compact(&mut self) {
        if self.sent == 0 {
            return;
        }
        if self.sent == self.bytes.len() {
            self.bytes.clear();
        } else if self.sent >= self.bytes.len() / 2 {
            self.bytes.drain(..self.sent);
        } else {
            return;
        }
        self.origin += self.sent as u64;
        self.sent = 0;
    }
}
