use crate::error::{Error, Result};
use crate::format::Format;

#[derive(Clone, Debug)]
pub(crate) struct Bytes<'d> {
    format: Format,
    data: &'d [u8],
    at: usize,
}

impl<'d> Bytes<'d> {
    pub(crate) fn new(format: Format, data: &'d [u8]) -> Bytes<'d> {
        Bytes {
            format,
            data,
            at: 0,
        }
    }

    pub(crate) fn short(&self) -> Error {
        Error::Truncated(self.format)
    }

    pub(crate) fn position(&self) -> usize {
        self.at
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len() - self.at
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    pub(crate) fn rest(&self) -> &'d [u8] {
        &self.data[self.at..]
    }

    pub(crate) fn seek(&mut self, at: usize) -> Result<()> {
        if at > self.data.len() {
            return Err(self.short());
        }
        self.at = at;
        Ok(())
    }

    pub(crate) fn exhaust(&mut self) {
        self.at = self.data.len();
    }

    pub(crate) fn skip(&mut self, len: usize) -> Result<()> {
        self.take(len).map(|_| ())
    }

    pub(crate) fn take(&mut self, len: usize) -> Result<&'d [u8]> {
        let bytes = self
            .data
            .get(self.at..)
            .and_then(|rest| rest.get(..len))
            .ok_or_else(|| self.short())?;
        self.at += len;
        Ok(bytes)
    }

    pub(crate) fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        let bytes = self.take(N)?;
        let mut array = [0; N];
        array.copy_from_slice(bytes);
        Ok(array)
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u16_be(&mut self) -> Result<u16> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub(crate) fn u32_be(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub(crate) fn i32_be(&mut self) -> Result<i32> {
        Ok(i32::from_be_bytes(self.array()?))
    }

    pub(crate) fn u64_be(&mut self) -> Result<u64> {
        Ok(u64::from_be_bytes(self.array()?))
    }

    pub(crate) fn u32_le(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    pub(crate) fn u64_le(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    pub(crate) fn i32_le(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub(crate) fn sized_be(&mut self, size: u8) -> Result<u64> {
        match size {
            0 => Ok(0),
            2 => self.u16_be().map(u64::from),
            4 => self.u32_be().map(u64::from),
            8 => self.u64_be(),
            _ => Err(Error::malformed(self.format, "field size")),
        }
    }

    pub(crate) fn fourcc(&mut self) -> Result<[u8; 4]> {
        self.array()
    }

    pub(crate) fn text(&mut self) -> Result<&'d [u8]> {
        let rest = self.rest();
        let end = rest
            .iter()
            .position(|&byte| byte == 0)
            .ok_or_else(|| self.short())?;
        self.at += end + 1;
        Ok(&rest[..end])
    }
}
