use crate::error::{Error, Result};

#[derive(Debug)]
pub struct Base32;

impl Base32 {
    pub fn encode(bytes: &[u8]) -> String {
        rikka_proton_api::base32::encode(bytes)
    }

    pub fn encode_padded(bytes: &[u8]) -> String {
        let mut encoded = Base32::encode(bytes);
        while !encoded.len().is_multiple_of(8) {
            encoded.push('=');
        }
        encoded
    }

    pub fn decode(text: &str) -> Result<Vec<u8>> {
        let text = text.trim_end_matches('=');
        let mut decoded = Vec::with_capacity(text.len() * 5 / 8);
        let mut bits = 0u16;
        let mut width = 0u32;
        for letter in text.chars() {
            let value = symbol(letter).ok_or(Error::MalformedBase32)?;
            bits = (bits << 5) | u16::from(value);
            width += 5;
            if width >= 8 {
                width -= 8;
                decoded.push((bits >> width) as u8);
            }
        }
        if bits & ((1 << width) - 1) != 0 {
            return Err(Error::MalformedBase32);
        }
        Ok(decoded)
    }
}

fn symbol(letter: char) -> Option<u8> {
    match letter {
        'A'..='Z' => Some(letter as u8 - b'A'),
        'a'..='z' => Some(letter as u8 - b'a'),
        '2'..='7' => Some(letter as u8 - b'2' + 26),
        _ => None,
    }
}
