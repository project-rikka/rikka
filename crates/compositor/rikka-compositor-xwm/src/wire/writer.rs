use std::os::fd::AsFd;

use rustix::io::Errno;

const SHORT_LIMIT: usize = u16::MAX as usize * 4;
const LENGTH_WORD: usize = 4;

#[derive(Debug, Default)]
pub(crate) struct Writer {
    buffer: Vec<u8>,
    flushed: usize,
}

pub(crate) fn wire_length(length: usize) -> usize {
    if length > SHORT_LIMIT {
        length + LENGTH_WORD
    } else {
        length
    }
}

impl Writer {
    pub(crate) fn push(&mut self, pieces: &[&[u8]], length: usize) {
        let [first, rest @ ..] = pieces else {
            return;
        };
        if length > SHORT_LIMIT {
            let words = u32::try_from((length + LENGTH_WORD) / 4).unwrap_or(u32::MAX);
            self.buffer.extend_from_slice(&first[..LENGTH_WORD]);
            self.buffer.extend_from_slice(&words.to_ne_bytes());
            self.buffer.extend_from_slice(&first[LENGTH_WORD..]);
        } else {
            self.buffer.extend_from_slice(first);
        }
        for piece in rest {
            self.buffer.extend_from_slice(piece);
        }
    }

    pub(crate) fn flush(&mut self, fd: impl AsFd) -> rustix::io::Result<bool> {
        while self.flushed < self.buffer.len() {
            match rustix::io::write(&fd, &self.buffer[self.flushed..]) {
                Ok(length) => self.flushed += length,
                Err(Errno::AGAIN) => return Ok(true),
                Err(Errno::INTR) => {}
                Err(error) => return Err(error),
            }
        }
        self.buffer.clear();
        self.flushed = 0;
        Ok(false)
    }
}
