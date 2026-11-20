use glam::{Vec2, Vec3, Vec4};

use crate::error::{Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Endian {
    Little,
    Big,
}

impl Endian {
    #[must_use]
    pub fn u16(self, bytes: [u8; 2]) -> u16 {
        match self {
            Self::Little => u16::from_le_bytes(bytes),
            Self::Big => u16::from_be_bytes(bytes),
        }
    }

    #[must_use]
    pub fn u32(self, bytes: [u8; 4]) -> u32 {
        match self {
            Self::Little => u32::from_le_bytes(bytes),
            Self::Big => u32::from_be_bytes(bytes),
        }
    }

    #[must_use]
    pub fn u64(self, bytes: [u8; 8]) -> u64 {
        match self {
            Self::Little => u64::from_le_bytes(bytes),
            Self::Big => u64::from_be_bytes(bytes),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ByteReader<'a> {
    bytes: &'a [u8],
    at: usize,
    endian: Endian,
}

impl<'a> ByteReader<'a> {
    #[must_use]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self::with_endian(bytes, Endian::Little)
    }

    #[must_use]
    pub fn with_endian(bytes: &'a [u8], endian: Endian) -> Self {
        Self {
            bytes,
            at: 0,
            endian,
        }
    }

    #[must_use]
    pub fn endian(&self) -> Endian {
        self.endian
    }

    #[must_use]
    pub fn position(&self) -> usize {
        self.at
    }

    pub fn seek(&mut self, at: usize) -> Result<()> {
        if at > self.bytes.len() {
            return Err(self.short(at - self.at.min(at)));
        }
        self.at = at;
        Ok(())
    }

    #[must_use]
    pub fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .at
            .checked_add(length)
            .ok_or_else(|| self.short(length))?;
        if end > self.bytes.len() {
            return Err(self.short(length));
        }
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut array = [0; N];
        array.copy_from_slice(self.take(N)?);
        Ok(array)
    }

    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16(&mut self) -> Result<u16> {
        Ok(self.endian.u16(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32> {
        Ok(self.endian.u32(self.array()?))
    }

    pub fn i16(&mut self) -> Result<i16> {
        Ok(self.u16()? as i16)
    }

    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }

    pub fn u64(&mut self) -> Result<u64> {
        Ok(self.endian.u64(self.array()?))
    }

    pub fn i64(&mut self) -> Result<i64> {
        Ok(self.u64()? as i64)
    }

    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_bits(self.u64()?))
    }

    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }

    pub fn c_string(&mut self) -> Result<&'a [u8]> {
        let rest = &self.bytes[self.at..];
        let length = rest
            .iter()
            .position(|&byte| byte == 0)
            .ok_or_else(|| self.short(1))?;
        self.at += length + 1;
        Ok(&rest[..length])
    }

    pub fn align(&mut self, alignment: usize) -> Result<()> {
        self.seek(self.at.next_multiple_of(alignment))
    }

    pub fn vec2(&mut self) -> Result<Vec2> {
        Ok(Vec2::new(self.f32()?, self.f32()?))
    }

    pub fn vec3(&mut self) -> Result<Vec3> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }

    pub fn vec4(&mut self) -> Result<Vec4> {
        Ok(Vec4::new(
            self.f32()?,
            self.f32()?,
            self.f32()?,
            self.f32()?,
        ))
    }

    pub fn count(&mut self, stride: usize, what: &str) -> Result<usize> {
        let count = self.i32()?;
        if count < 0 {
            return Err(Error::Damaged(format!("declared {count} {what}")));
        }
        let count = count as usize;
        if stride > 0 && count.saturating_mul(stride) > self.remaining() {
            return Err(Error::Damaged(format!(
                "declared {count} {what} exceed the remaining {} bytes",
                self.remaining()
            )));
        }
        Ok(count)
    }

    fn short(&self, missing: usize) -> Error {
        Error::Damaged(format!(
            "unexpected end of file after {} bytes: {missing} more expected at {}",
            self.bytes.len(),
            self.at
        ))
    }
}

#[derive(Debug, Default)]
pub struct ByteWriter {
    bytes: Vec<u8>,
}

impl ByteWriter {
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    #[must_use]
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn fixed(&mut self, bytes: &[u8], length: usize) {
        let taken = bytes.len().min(length);
        self.bytes.extend_from_slice(&bytes[..taken]);
        self.bytes.resize(self.bytes.len() + length - taken, 0);
    }

    pub fn fill(&mut self, byte: u8, length: usize) {
        self.bytes.resize(self.bytes.len() + length, byte);
    }

    pub fn align(&mut self, alignment: usize, byte: u8) {
        self.bytes
            .resize(self.bytes.len().next_multiple_of(alignment), byte);
    }

    pub fn set_u32(&mut self, at: usize, value: u32) {
        self.bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }

    pub fn set_u64(&mut self, at: usize, value: u64) {
        self.bytes[at..at + 8].copy_from_slice(&value.to_le_bytes());
    }

    pub fn i16(&mut self, value: i16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn f32s(&mut self, values: &[f32]) {
        self.bytes.reserve(values.len() * 4);
        for value in values {
            self.bytes.extend_from_slice(&value.to_le_bytes());
        }
    }

    pub fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn i32(&mut self, value: i32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn f32(&mut self, value: f32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub fn vec2(&mut self, value: Vec2) {
        self.f32(value.x);
        self.f32(value.y);
    }

    pub fn vec3(&mut self, value: Vec3) {
        self.f32(value.x);
        self.f32(value.y);
        self.f32(value.z);
    }

    pub fn vec4(&mut self, value: Vec4) {
        self.f32(value.x);
        self.f32(value.y);
        self.f32(value.z);
        self.f32(value.w);
    }
}
