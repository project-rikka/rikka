use std::fmt::{self, Write as _};

const DIGITS: &[u8; 16] = b"0123456789abcdef";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidHex {
    Length,
    Digit,
}

impl fmt::Display for InvalidHex {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            InvalidHex::Length => "hexadecimal text has the wrong length",
            InvalidHex::Digit => "text is not hexadecimal",
        })
    }
}

impl std::error::Error for InvalidHex {}

#[derive(Clone, Copy, Debug)]
pub struct Hex<'a>(pub &'a [u8]);

impl fmt::Display for Hex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            formatter.write_char(char::from(DIGITS[usize::from(byte >> 4)]))?;
            formatter.write_char(char::from(DIGITS[usize::from(byte & 0x0f)]))?;
        }
        Ok(())
    }
}

#[must_use]
pub fn encode(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

pub fn decode(text: impl AsRef<[u8]>) -> Result<Vec<u8>, InvalidHex> {
    let text = text.as_ref();
    let mut bytes = vec![0; text.len() / 2];
    decode_to_slice(text, &mut bytes)?;
    Ok(bytes)
}

pub fn decode_to_slice(text: impl AsRef<[u8]>, bytes: &mut [u8]) -> Result<(), InvalidHex> {
    let text = text.as_ref();
    if text.len() != bytes.len() * 2 {
        return Err(InvalidHex::Length);
    }
    let (pairs, _) = text.as_chunks::<2>();
    for (byte, [high, low]) in bytes.iter_mut().zip(pairs) {
        *byte = digit(*high)? << 4 | digit(*low)?;
    }
    Ok(())
}

fn digit(character: u8) -> Result<u8, InvalidHex> {
    match character {
        b'0'..=b'9' => Ok(character - b'0'),
        b'a'..=b'f' => Ok(character - b'a' + 10),
        b'A'..=b'F' => Ok(character - b'A' + 10),
        _ => Err(InvalidHex::Digit),
    }
}
