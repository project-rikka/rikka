pub(crate) struct BitReader<'d> {
    data: &'d [u8],
    position: usize,
    buffer: u64,
    count: u32,
    padding: u32,
}

impl<'d> BitReader<'d> {
    pub(crate) fn new(data: &'d [u8]) -> Self {
        Self {
            data,
            position: 0,
            buffer: 0,
            count: 0,
            padding: 0,
        }
    }

    #[inline]
    pub(crate) fn refill(&mut self) {
        if let Some(bytes) = self.data.get(self.position..self.position + 8) {
            let word = u64::from_le_bytes(bytes.try_into().unwrap_or([0; 8]));
            self.buffer |= word << self.count;
            self.position += ((63 - self.count) >> 3) as usize;
            self.count |= 56;
        } else {
            self.refill_tail();
        }
    }

    fn refill_tail(&mut self) {
        while self.count < 56 {
            let byte = match self.data.get(self.position) {
                Some(&byte) => {
                    self.position += 1;
                    byte
                }
                None => {
                    self.padding += 1;
                    0
                }
            };
            self.buffer |= u64::from(byte) << self.count;
            self.count += 8;
        }
    }

    #[inline]
    pub(crate) fn peek(&self) -> u64 {
        self.buffer
    }

    #[inline]
    pub(crate) fn consume(&mut self, bits: u32) {
        self.buffer >>= bits;
        self.count -= bits;
    }

    #[inline]
    pub(crate) fn take(&mut self, bits: u32) -> u32 {
        let value = (self.buffer & ((1u64 << bits) - 1)) as u32;
        self.consume(bits);
        value
    }

    #[inline]
    pub(crate) fn read(&mut self, bits: u32) -> u32 {
        self.refill();
        self.take(bits)
    }

    #[inline]
    pub(crate) fn is_overrun(&self) -> bool {
        self.padding * 8 > self.count
    }

    pub(crate) fn align_to_byte(&mut self) {
        let extra = self.count % 8;
        self.consume(extra);
    }

    pub(crate) fn byte_position_rounded_up(&self) -> Option<usize> {
        if self.is_overrun() {
            return None;
        }
        let consumed = (self.position + self.padding as usize) * 8 - self.count as usize;
        Some(consumed.div_ceil(8))
    }

    pub(crate) fn byte_position(&self) -> Option<usize> {
        if self.is_overrun() {
            return None;
        }
        let buffered = (self.count / 8) as usize;
        Some(self.position + self.padding as usize - buffered)
    }

    pub(crate) fn skip_bytes_from(&mut self, start: usize, len: usize) -> Option<&'d [u8]> {
        let bytes = self.data.get(start..start.checked_add(len)?)?;
        self.position = start + len;
        self.buffer = 0;
        self.count = 0;
        self.padding = 0;
        Some(bytes)
    }
}
