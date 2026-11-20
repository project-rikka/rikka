pub(crate) struct BitWriter<'o> {
    out: &'o mut Vec<u8>,
    buffer: u64,
    count: u32,
}

impl<'o> BitWriter<'o> {
    pub(crate) fn new(out: &'o mut Vec<u8>) -> Self {
        Self {
            out,
            buffer: 0,
            count: 0,
        }
    }

    #[inline]
    pub(crate) fn put(&mut self, value: u32, bits: u32) {
        self.buffer |= u64::from(value) << self.count;
        self.count += bits;
        if self.count >= 32 {
            self.out
                .extend_from_slice(&(self.buffer as u32).to_le_bytes());
            self.buffer >>= 32;
            self.count -= 32;
        }
    }

    pub(crate) fn align_to_byte(&mut self) {
        self.count = self.count.next_multiple_of(8);
        while self.count >= 8 {
            self.out.push(self.buffer as u8);
            self.buffer >>= 8;
            self.count -= 8;
        }
    }

    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.align_to_byte();
        self.out.extend_from_slice(bytes);
    }
}
