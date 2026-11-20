const DEPTH: usize = 8;

#[derive(Debug)]
pub(crate) struct Lookahead {
    ring: [u64; DEPTH],
    head: usize,
    len: usize,
}

impl Lookahead {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Self {
            ring: [0; DEPTH],
            head: 0,
            len: 0,
        }
    }

    #[inline(always)]
    pub(crate) fn push(&mut self, key: u64, mut ready: impl FnMut(u64)) {
        if self.len == DEPTH {
            ready(self.ring[self.head]);
            self.ring[self.head] = key;
            self.head = (self.head + 1) % DEPTH;
        } else {
            self.ring[(self.head + self.len) % DEPTH] = key;
            self.len += 1;
        }
    }

    #[inline(always)]
    pub(crate) fn drain(&mut self, mut ready: impl FnMut(u64)) {
        while self.len > 0 {
            ready(self.ring[self.head]);
            self.head = (self.head + 1) % DEPTH;
            self.len -= 1;
        }
    }
}
