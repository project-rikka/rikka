use crate::error::DecodeError;

pub(crate) struct Writer<'a>(pub(crate) &'a mut Vec<u8>);

impl Writer<'_> {
    pub(crate) fn u8(&mut self, value: u8) -> &mut Self {
        self.0.push(value);
        self
    }

    pub(crate) fn bool(&mut self, value: bool) -> &mut Self {
        self.u8(u8::from(value))
    }

    pub(crate) fn u16(&mut self, value: u16) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub(crate) fn i16(&mut self, value: i16) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub(crate) fn u32(&mut self, value: u32) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub(crate) fn i32(&mut self, value: i32) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub(crate) fn u64(&mut self, value: u64) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub(crate) fn f32(&mut self, value: f32) -> &mut Self {
        self.0.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub(crate) fn str(&mut self, value: &str) -> &mut Self {
        let bytes = truncated(value, usize::from(u16::MAX));
        self.u16(bytes.len() as u16);
        self.0.extend_from_slice(bytes);
        self
    }

    pub(crate) fn bytes(&mut self, value: &[u8]) -> &mut Self {
        self.u32(u32::try_from(value.len()).unwrap_or(u32::MAX));
        self.raw(value)
    }

    pub(crate) fn raw(&mut self, value: &[u8]) -> &mut Self {
        self.0.extend_from_slice(value);
        self
    }
}

fn truncated(text: &str, most: usize) -> &[u8] {
    if text.len() <= most {
        return text.as_bytes();
    }
    let mut end = most;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text.as_bytes()[..end]
}

pub(crate) struct Reader<'a> {
    kind: u8,
    rest: &'a [u8],
}

impl<'a> Reader<'a> {
    pub(crate) fn new(kind: u8, rest: &'a [u8]) -> Reader<'a> {
        Reader { kind, rest }
    }

    pub(crate) fn consumed_from(&self, start: &[u8]) -> usize {
        start.len() - self.rest.len()
    }

    fn take<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let (head, rest) = self
            .rest
            .split_first_chunk::<N>()
            .ok_or(DecodeError::Truncated(self.kind))?;
        self.rest = rest;
        Ok(*head)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take::<1>()?[0])
    }

    pub(crate) fn bool(&mut self) -> Result<bool, DecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            other => Err(DecodeError::InvalidValue {
                field: "flag",
                value: u64::from(other),
            }),
        }
    }

    pub(crate) fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.take()?))
    }

    pub(crate) fn i16(&mut self) -> Result<i16, DecodeError> {
        Ok(i16::from_le_bytes(self.take()?))
    }

    pub(crate) fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    pub(crate) fn i32(&mut self) -> Result<i32, DecodeError> {
        Ok(i32::from_le_bytes(self.take()?))
    }

    pub(crate) fn u64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(self.take()?))
    }

    pub(crate) fn f32(&mut self) -> Result<f32, DecodeError> {
        let value = f32::from_le_bytes(self.take()?);
        match value.is_finite() {
            true => Ok(value),
            false => Err(DecodeError::NonFinite(self.kind)),
        }
    }

    pub(crate) fn raw(&mut self, length: usize) -> Result<&'a [u8], DecodeError> {
        if self.rest.len() < length {
            return Err(DecodeError::Truncated(self.kind));
        }
        let (head, rest) = self.rest.split_at(length);
        self.rest = rest;
        Ok(head)
    }

    pub(crate) fn str(&mut self) -> Result<&'a str, DecodeError> {
        let length = usize::from(self.u16()?);
        let bytes = self.raw(length)?;
        std::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidText(self.kind))
    }

    pub(crate) fn bytes(&mut self) -> Result<&'a [u8], DecodeError> {
        let length = self.u32()? as usize;
        self.raw(length)
    }

    pub(crate) fn rest(&mut self) -> &'a [u8] {
        std::mem::take(&mut self.rest)
    }

    pub(crate) fn finish(self) -> Result<(), DecodeError> {
        match self.rest.is_empty() {
            true => Ok(()),
            false => Err(DecodeError::Trailing(self.kind)),
        }
    }
}
