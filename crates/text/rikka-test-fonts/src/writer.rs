const FIXED_ONE: f32 = 65536.0;
const F2DOT14_ONE: f32 = 16384.0;

#[derive(Clone, Debug, Default)]
pub struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn finish(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.bytes)
    }

    pub fn u8(&mut self, value: u8) -> &mut Self {
        self.bytes.push(value);
        self
    }

    pub fn i8(&mut self, value: i8) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }

    pub fn u16(&mut self, value: u16) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }

    pub fn i16(&mut self, value: i16) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }

    pub fn u24(&mut self, value: usize) -> &mut Self {
        self.raw(&u24_bytes(value))
    }

    pub fn u32(&mut self, value: u32) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }

    pub fn i32(&mut self, value: i32) -> &mut Self {
        self.raw(&value.to_be_bytes())
    }

    pub fn fixed(&mut self, value: f32) -> &mut Self {
        self.i32((value * FIXED_ONE) as i32)
    }

    pub fn f2dot14(&mut self, value: f32) -> &mut Self {
        self.i16((value * F2DOT14_ONE).round() as i16)
    }

    pub fn raw(&mut self, bytes: &[u8]) -> &mut Self {
        self.bytes.extend_from_slice(bytes);
        self
    }

    pub fn align(&mut self, alignment: usize) -> &mut Self {
        self.bytes
            .resize(self.bytes.len().next_multiple_of(alignment), 0);
        self
    }

    pub fn set_u8(&mut self, at: usize, value: u8) {
        self.bytes[at] = value;
    }

    pub fn set_u16(&mut self, at: usize, value: usize) {
        self.bytes[at..at + 2].copy_from_slice(&(value as u16).to_be_bytes());
    }

    pub fn set_u24(&mut self, at: usize, value: usize) {
        self.bytes[at..at + 3].copy_from_slice(&u24_bytes(value));
    }

    pub fn set_u32(&mut self, at: usize, value: usize) {
        self.bytes[at..at + 4].copy_from_slice(&(value as u32).to_be_bytes());
    }
}

fn u24_bytes(value: usize) -> [u8; 3] {
    let [_, high, middle, low] = (value as u32).to_be_bytes();
    [high, middle, low]
}
