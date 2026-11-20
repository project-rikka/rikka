use std::collections::VecDeque;
use std::os::fd::OwnedFd;

use crate::ids::MAX_MESSAGE_SIZE;

#[derive(Debug)]
pub(crate) struct Incoming {
    buffer: Box<[u8]>,
    start: usize,
    end: usize,
    limit: usize,
    pub(crate) fds: VecDeque<OwnedFd>,
}

impl Incoming {
    pub(crate) fn new(initial: usize, limit: usize) -> Self {
        Self {
            buffer: vec![0; initial].into_boxed_slice(),
            start: 0,
            end: 0,
            limit,
            fds: VecDeque::new(),
        }
    }

    pub(crate) fn capacity(&self) -> usize {
        self.buffer.len()
    }

    pub(crate) fn len(&self) -> usize {
        self.end - self.start
    }

    pub(crate) fn spare(&mut self) -> Option<&mut [u8]> {
        if self.start > 0 && self.capacity() - self.end < MAX_MESSAGE_SIZE {
            self.buffer.copy_within(self.start..self.end, 0);
            self.end -= self.start;
            self.start = 0;
        }
        if self.end == self.capacity() && !self.grow() {
            return None;
        }
        Some(&mut self.buffer[self.end..])
    }

    pub(crate) fn commit(&mut self, count: usize) {
        self.end = (self.end + count).min(self.capacity());
    }

    pub(crate) fn header(&self) -> Option<[u32; 2]> {
        let bytes = self.buffer.get(self.start..self.start + 8)?;
        (self.len() >= 8).then(|| {
            [
                u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                u32::from_ne_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            ]
        })
    }

    pub(crate) fn message(&mut self, size: usize) -> (&[u8], &mut VecDeque<OwnedFd>) {
        (
            &self.buffer[self.start + 8..self.start + size],
            &mut self.fds,
        )
    }

    pub(crate) fn body(&self, size: usize) -> &[u8] {
        &self.buffer[self.start + 8..self.start + size]
    }

    pub(crate) fn consume(&mut self, count: usize) {
        self.start = (self.start + count).min(self.end);
        if self.start == self.end {
            self.start = 0;
            self.end = 0;
        }
    }

    pub(crate) fn clear(&mut self) {
        self.start = 0;
        self.end = 0;
        self.fds.clear();
    }

    fn grow(&mut self) -> bool {
        let capacity = self.capacity();
        if capacity >= self.limit {
            return false;
        }
        let mut grown = std::mem::take(&mut self.buffer).into_vec();
        grown.resize((capacity * 2).min(self.limit), 0);
        self.buffer = grown.into_boxed_slice();
        true
    }
}
