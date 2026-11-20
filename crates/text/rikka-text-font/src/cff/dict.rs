use crate::parse::bytes::Bytes;

const MAX_OPERANDS: usize = 513;
const ESCAPE: u8 = 12;

pub(crate) mod op {
    pub(crate) const CHARSET: u16 = 15;
    pub(crate) const CHAR_STRINGS: u16 = 17;
    pub(crate) const PRIVATE: u16 = 18;
    pub(crate) const SUBRS: u16 = 19;
    pub(crate) const DEFAULT_WIDTH_X: u16 = 20;
    pub(crate) const NOMINAL_WIDTH_X: u16 = 21;
    pub(crate) const VSINDEX: u16 = 22;
    pub(crate) const VSTORE: u16 = 24;
    pub(crate) const CHARSTRING_TYPE: u16 = 0x0C06;
    pub(crate) const FONT_MATRIX: u16 = 0x0C07;
    pub(crate) const ROS: u16 = 0x0C1E;
    pub(crate) const FD_ARRAY: u16 = 0x0C24;
    pub(crate) const FD_SELECT: u16 = 0x0C25;
}

pub(crate) fn parse_dict(data: &[u8], mut entry: impl FnMut(u16, &[f64])) -> Option<()> {
    let mut stack = [0.0f64; MAX_OPERANDS];
    let mut len = 0;
    let mut position = 0;
    while position < data.len() {
        let b0 = data.u8_at(position)?;
        position += 1;
        let value = match b0 {
            0..=21 => {
                let operator = if b0 == ESCAPE {
                    let b1 = data.u8_at(position)?;
                    position += 1;
                    0x0C00 | u16::from(b1)
                } else {
                    u16::from(b0)
                };
                entry(operator, &stack[..len]);
                len = 0;
                continue;
            }
            22..=27 => {
                entry(u16::from(b0), &stack[..len]);
                len = 0;
                continue;
            }
            28 => {
                let value = data.i16_at(position)?;
                position += 2;
                f64::from(value)
            }
            29 => {
                let value = data.i32_at(position)?;
                position += 4;
                f64::from(value)
            }
            30 => {
                let (value, used) = parse_real(data.tail(position)?)?;
                position += used;
                value
            }
            32..=246 => f64::from(i16::from(b0) - 139),
            247..=250 => {
                let b1 = data.u8_at(position)?;
                position += 1;
                f64::from((i16::from(b0) - 247) * 256 + i16::from(b1) + 108)
            }
            251..=254 => {
                let b1 = data.u8_at(position)?;
                position += 1;
                f64::from(-(i16::from(b0) - 251) * 256 - i16::from(b1) - 108)
            }
            _ => return None,
        };
        if len >= MAX_OPERANDS {
            return None;
        }
        stack[len] = value;
        len += 1;
    }
    Some(())
}

fn parse_real(data: &[u8]) -> Option<(f64, usize)> {
    let mut text = [0u8; 64];
    let mut length = 0;
    let mut push = |bytes: &[u8]| -> Option<()> {
        let end = length + bytes.len();
        text.get_mut(length..end)?.copy_from_slice(bytes);
        length = end;
        Some(())
    };
    for (index, &byte) in data.iter().enumerate() {
        for nibble in [byte >> 4, byte & 0x0F] {
            match nibble {
                0..=9 => push(&[b'0' + nibble])?,
                0x0A => push(b".")?,
                0x0B => push(b"E")?,
                0x0C => push(b"E-")?,
                0x0E => push(b"-")?,
                0x0F => {
                    let value = std::str::from_utf8(&text[..length]).ok()?;
                    let value = if value.is_empty() {
                        0.0
                    } else {
                        value.parse::<f64>().ok()?
                    };
                    return Some((value, index + 1));
                }
                _ => return None,
            }
        }
    }
    None
}
