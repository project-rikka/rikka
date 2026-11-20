use std::os::fd::AsFd;

use rustix::buffer::spare_capacity;
use rustix::io::Errno;
use x11rb_protocol::protocol::xproto::GE_GENERIC_EVENT;

const HEADER: usize = 32;
const REPLY: u8 = 1;
const READ_SIZE: usize = 64 * 1024;
const LARGE: usize = READ_SIZE / 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Received {
    Nothing,
    Some,
    Closed,
}

#[derive(Debug)]
pub(crate) struct Reader {
    buffer: Vec<u8>,
    consumed: usize,
    large: Option<Large>,
}

#[derive(Debug)]
struct Large {
    packet: Vec<u8>,
    length: usize,
}

impl Default for Reader {
    fn default() -> Self {
        Self {
            buffer: Vec::with_capacity(READ_SIZE),
            consumed: 0,
            large: None,
        }
    }
}

impl Reader {
    pub(crate) fn receive(
        &mut self,
        fd: impl AsFd,
        mut deliver: impl FnMut(Vec<u8>),
    ) -> rustix::io::Result<Received> {
        let mut received = Received::Nothing;
        loop {
            let read = match self.large.as_mut() {
                Some(large) => rustix::io::read(&fd, spare_capacity(&mut large.packet)),
                None => {
                    self.make_room();
                    rustix::io::read(&fd, spare_capacity(&mut self.buffer))
                }
            };
            match read {
                Ok(0) => return Ok(Received::Closed),
                Ok(_) => received = Received::Some,
                Err(Errno::AGAIN) => return Ok(received),
                Err(Errno::INTR) => continue,
                Err(error) => return Err(error),
            }
            self.split(&mut deliver);
        }
    }

    fn make_room(&mut self) {
        if self.consumed > 0 {
            self.buffer.drain(..self.consumed);
            self.consumed = 0;
        }
        if self.buffer.capacity() - self.buffer.len() < HEADER {
            self.buffer.reserve(READ_SIZE);
        }
    }

    fn split(&mut self, deliver: &mut impl FnMut(Vec<u8>)) {
        if let Some(large) = self.large.as_mut() {
            if large.packet.len() < large.length {
                return;
            }
            let mut large = self.large.take().expect("a large packet is being read");
            let excess = large.packet.split_off(large.length);
            self.buffer.extend_from_slice(&excess);
            deliver(large.packet);
        }
        loop {
            let available = &self.buffer[self.consumed..];
            if available.len() < HEADER {
                return;
            }
            let length = packet_length(available);
            if available.len() >= length {
                deliver(available[..length].to_vec());
                self.consumed += length;
                continue;
            }
            if length >= LARGE {
                let mut packet = Vec::with_capacity(length);
                packet.extend_from_slice(available);
                self.buffer.clear();
                self.consumed = 0;
                self.large = Some(Large { packet, length });
            }
            return;
        }
    }
}

fn packet_length(header: &[u8]) -> usize {
    let counted = header[0] == REPLY || header[0] & 0x7f == GE_GENERIC_EVENT;
    if !counted {
        return HEADER;
    }
    let words = u32::from_ne_bytes([header[4], header[5], header[6], header[7]]);
    HEADER + words as usize * 4
}
