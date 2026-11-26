use crate::error::{Error, Result};

pub(crate) fn decode(bytes: &[u8]) -> Result<String> {
    if bytes.iter().all(|byte| *byte != 0 && *byte < 0x80) {
        return String::from_utf8(bytes.to_vec()).map_err(|_| Error::Text);
    }
    let mut units = Vec::with_capacity(bytes.len());
    let mut index = 0;
    let continuation = |at: usize| match bytes.get(at) {
        Some(byte) if byte & 0xC0 == 0x80 => Ok(u16::from(byte & 0x3F)),
        _ => Err(Error::Text),
    };
    while let Some(&first) = bytes.get(index) {
        match first {
            0x01..=0x7F => {
                units.push(u16::from(first));
                index += 1;
            }
            0xC0..=0xDF => {
                units.push((u16::from(first & 0x1F) << 6) | continuation(index + 1)?);
                index += 2;
            }
            0xE0..=0xEF => {
                units.push(
                    (u16::from(first & 0x0F) << 12)
                        | (continuation(index + 1)? << 6)
                        | continuation(index + 2)?,
                );
                index += 3;
            }
            _ => return Err(Error::Text),
        }
    }
    String::from_utf16(&units).map_err(|_| Error::Text)
}

pub(crate) fn encode(text: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(text.len());
    for unit in text.encode_utf16() {
        match unit {
            0x0001..=0x007F => bytes.push(unit as u8),
            0x0000 | 0x0080..=0x07FF => {
                bytes.push(0xC0 | (unit >> 6) as u8);
                bytes.push(0x80 | (unit & 0x3F) as u8);
            }
            _ => {
                bytes.push(0xE0 | (unit >> 12) as u8);
                bytes.push(0x80 | ((unit >> 6) & 0x3F) as u8);
                bytes.push(0x80 | (unit & 0x3F) as u8);
            }
        }
    }
    bytes
}
