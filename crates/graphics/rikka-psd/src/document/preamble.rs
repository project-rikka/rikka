use std::io::Read;

use super::header::FileHeader;
use crate::error::{Error, Result};
use crate::io::Reader;
use crate::resources::Resources;

#[derive(Debug, Clone, PartialEq)]
pub struct Preamble {
    pub header: FileHeader,
    pub resources: Resources,
}

impl Preamble {
    pub fn read(source: &mut impl Read) -> Result<Preamble> {
        let mut offset = 0u64;
        let header = FileHeader::read(&exactly(source, FileHeader::LEN as u64, &mut offset)?)?;
        let color_length = section_length(source, &mut offset)?;
        exactly(source, color_length, &mut offset)?;
        let resources_length = section_length(source, &mut offset)?;
        let bytes = exactly(source, resources_length, &mut offset)?;
        let resources = Resources::read(&mut Reader::new(&bytes))?;
        Ok(Preamble { header, resources })
    }
}

fn section_length(source: &mut impl Read, offset: &mut u64) -> Result<u64> {
    let bytes = exactly(source, 4, offset)?;
    Ok(u64::from(u32::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3],
    ])))
}

fn exactly(source: &mut impl Read, length: u64, offset: &mut u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    source.by_ref().take(length).read_to_end(&mut bytes)?;
    if (bytes.len() as u64) < length {
        return Err(Error::Truncated {
            offset: *offset + bytes.len() as u64,
            needed: length - bytes.len() as u64,
        });
    }
    *offset += length;
    Ok(bytes)
}
