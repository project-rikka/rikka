pub(crate) mod bmp;
pub(crate) mod dds;
pub(crate) mod farbfeld;
pub(crate) mod ffmpeg;
pub(crate) mod foreign;
pub(crate) mod gif;
pub(crate) mod hdr;
pub(crate) mod ico;
pub(crate) mod png;
pub(crate) mod pnm;
pub(crate) mod qoi;
pub(crate) mod tga;
pub(crate) mod webp;

use crate::color_type::ColorType;
use crate::error::{Error, Result};
use crate::format::Format;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Limits {
    pub(crate) max_alloc: Option<u64>,
    pub(crate) max_dimensions: Option<(u32, u32)>,
}

impl Limits {
    pub(crate) fn admit(&self, width: u32, height: u32, color: ColorType) -> Result<usize> {
        if width == 0 || height == 0 {
            return Err(Error::Decoding(format!("{width}x{height} image is empty")));
        }
        if let Some((most_wide, most_tall)) = self.max_dimensions
            && (width > most_wide || height > most_tall)
        {
            return Err(Error::LimitsExceeded(format!(
                "{width}x{height} image exceeds {most_wide}x{most_tall}"
            )));
        }
        let bytes = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|pixels| pixels.checked_mul(u64::from(color.bytes_per_pixel())))
            .filter(|&bytes| bytes <= isize::MAX as u64)
            .ok_or_else(|| {
                Error::LimitsExceeded(format!("{width}x{height} image is too large to address"))
            })?;
        self.admit_bytes(bytes)?;
        Ok(bytes as usize)
    }

    pub(crate) fn admit_bytes(&self, bytes: u64) -> Result<()> {
        match self.max_alloc {
            Some(most) if bytes > most => Err(Error::LimitsExceeded(format!(
                "{bytes} bytes exceed the limit of {most}"
            ))),
            _ => Ok(()),
        }
    }
}

const UNQUESTIONED_BYTES: usize = 64 << 20;

pub(crate) fn plausible(
    format: Format,
    decoded: usize,
    encoded: usize,
    most_expansion: usize,
) -> Result<()> {
    if decoded > UNQUESTIONED_BYTES && decoded / most_expansion > encoded {
        return Err(damaged(format, "file ends early"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Probe {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) color: ColorType,
}

pub(crate) fn damaged(format: Format, problem: &str) -> Error {
    Error::Decoding(format!("{format}: {problem}"))
}

pub(crate) fn unsupported(format: Format, feature: &str) -> Error {
    Error::Unsupported(format!("{format}: {feature}"))
}

#[derive(Clone, Debug)]
pub(crate) struct Reader<'d> {
    format: Format,
    data: &'d [u8],
    at: usize,
}

impl<'d> Reader<'d> {
    pub(crate) fn new(format: Format, data: &'d [u8]) -> Self {
        Self {
            format,
            data,
            at: 0,
        }
    }

    pub(crate) fn short(&self) -> Error {
        damaged(self.format, "file ends early")
    }

    pub(crate) fn position(&self) -> usize {
        self.at
    }

    pub(crate) fn remaining(&self) -> usize {
        self.data.len() - self.at
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
        Ok(bytes.try_into().unwrap_or([0; N]))
    }

    pub(crate) fn u8(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    pub(crate) fn u16_le(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32_le(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    pub(crate) fn i32_le(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.array()?))
    }

    pub(crate) fn u32_be(&mut self) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array()?))
    }
}
