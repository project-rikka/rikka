use crate::error::{Error, Result};
use crate::format::Format;

const FORMAT: Format = Format::JpegXl;
const ENUMERATION: [Distribution; 4] = [
    Distribution::Value(0),
    Distribution::Value(1),
    Distribution::Bits(4, 2),
    Distribution::Bits(6, 18),
];

#[derive(Clone, Copy, Debug)]
pub(crate) enum Distribution {
    Value(u32),
    Bits(u32, u32),
}

#[derive(Debug)]
pub(crate) struct Bits<'d> {
    data: &'d [u8],
    at: u64,
}

impl<'d> Bits<'d> {
    pub(crate) fn new(data: &'d [u8]) -> Bits<'d> {
        Bits { data, at: 0 }
    }

    pub(crate) fn read(&mut self, count: u32) -> Result<u32> {
        let mut value = 0u32;
        for bit in 0..count {
            let byte = self
                .data
                .get((self.at / 8) as usize)
                .ok_or(Error::Truncated(FORMAT))?;
            value |= u32::from((byte >> (self.at % 8)) & 1) << bit;
            self.at += 1;
        }
        Ok(value)
    }

    pub(crate) fn flag(&mut self) -> Result<bool> {
        Ok(self.read(1)? == 1)
    }

    pub(crate) fn choose(&mut self, choices: [Distribution; 4]) -> Result<u32> {
        match choices[self.read(2)? as usize] {
            Distribution::Value(value) => Ok(value),
            Distribution::Bits(count, offset) => self
                .read(count)?
                .checked_add(offset)
                .ok_or_else(|| Error::malformed(FORMAT, "header field")),
        }
    }

    pub(crate) fn enumerated(&mut self) -> Result<u32> {
        self.choose(ENUMERATION)
    }

    pub(crate) fn skip(&mut self, count: u64) -> Result<()> {
        let end = self
            .at
            .checked_add(count)
            .filter(|&end| end <= self.data.len() as u64 * 8)
            .ok_or(Error::Truncated(FORMAT))?;
        self.at = end;
        Ok(())
    }
}
