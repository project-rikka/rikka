use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::io::mac_roman;
use crate::key::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Width {
    Four,
    Eight,
}

impl Width {
    pub(crate) fn from_wide(wide: bool) -> Self {
        if wide { Width::Eight } else { Width::Four }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Padding {
    Counted(usize),
    Trailing(usize),
}

#[derive(Default)]
pub(crate) struct Writer {
    buffer: Vec<u8>,
}

impl Writer {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.buffer.len()
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }

    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    pub(crate) fn zeros(&mut self, count: usize) {
        self.buffer.resize(self.buffer.len() + count, 0);
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.buffer.push(value);
    }

    pub(crate) fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.bytes(&value.to_be_bytes());
    }

    pub(crate) fn i16(&mut self, value: i16) {
        self.bytes(&value.to_be_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.bytes(&value.to_be_bytes());
    }

    pub(crate) fn i32(&mut self, value: i32) {
        self.bytes(&value.to_be_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.bytes(&value.to_be_bytes());
    }

    pub(crate) fn i64(&mut self, value: i64) {
        self.bytes(&value.to_be_bytes());
    }

    pub(crate) fn f32(&mut self, value: f32) {
        self.bytes(&value.to_be_bytes());
    }

    pub(crate) fn f64(&mut self, value: f64) {
        self.bytes(&value.to_be_bytes());
    }

    pub(crate) fn key(&mut self, key: Key) {
        self.bytes(&key.0);
    }

    pub(crate) fn code(&mut self, code: &[u8; 4]) {
        self.bytes(code);
    }

    pub(crate) fn length(&mut self, value: u64, width: Width) -> Result<()> {
        match width {
            Width::Four => self.u32(u32::try_from(value).map_err(|_| Error::Oversized)?),
            Width::Eight => self.u64(value),
        }
        Ok(())
    }

    pub(crate) fn rect(&mut self, rect: Rect) {
        self.i32(rect.top);
        self.i32(rect.left);
        self.i32(rect.bottom);
        self.i32(rect.right);
    }

    pub(crate) fn fixed16(&mut self, value: f64) {
        self.i32((value * 65536.0).round() as i32);
    }

    pub(crate) fn pascal_bytes(&mut self, text: &[u8], alignment: usize) {
        let length = text.len().min(255);
        self.u8(length as u8);
        self.bytes(&text[..length]);
        let consumed = length + 1;
        let alignment = alignment.max(1);
        self.zeros(consumed.div_ceil(alignment) * alignment - consumed);
    }

    pub(crate) fn pascal(&mut self, text: &str, alignment: usize) {
        self.pascal_bytes(&mac_roman::encode(text), alignment);
    }

    pub(crate) fn utf16(&mut self, text: &str) {
        for unit in text.encode_utf16() {
            self.u16(unit);
        }
    }

    pub(crate) fn unicode(&mut self, text: &str) {
        self.u32(text.encode_utf16().count() as u32);
        self.utf16(text);
    }

    pub(crate) fn unicode_terminated(&mut self, text: &str) {
        self.u32(text.encode_utf16().count() as u32 + 1);
        self.utf16(text);
        self.u16(0);
    }

    pub(crate) fn align_from(&mut self, start: usize, alignment: usize) {
        let body_len = self.len() - start;
        let padded = body_len.div_ceil(alignment) * alignment;
        self.zeros(padded - body_len);
    }

    pub(crate) fn section(
        &mut self,
        width: Width,
        padding: Padding,
        body: impl FnOnce(&mut Writer) -> Result<()>,
    ) -> Result<()> {
        let at = self.len();
        self.length(0, width)?;
        let start = self.len();
        body(self)?;
        let content = self.len() - start;
        let (alignment, counted) = match padding {
            Padding::Counted(alignment) => (alignment, true),
            Padding::Trailing(alignment) => (alignment, false),
        };
        let padded = content.div_ceil(alignment.max(1)) * alignment.max(1);
        self.zeros(padded - content);
        let recorded = if counted { padded } else { content } as u64;
        let bytes = match width {
            Width::Four => u32::try_from(recorded)
                .map_err(|_| Error::Oversized)?
                .to_be_bytes()
                .to_vec(),
            Width::Eight => recorded.to_be_bytes().to_vec(),
        };
        self.buffer[at..at + bytes.len()].copy_from_slice(&bytes);
        Ok(())
    }
}
