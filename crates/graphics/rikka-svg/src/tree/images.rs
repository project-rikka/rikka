use std::sync::Arc;

use rikka_vector::Pixmap;

use crate::document::parse_nested;
use crate::links::linked_bytes;
use crate::resources::Sources;
use crate::tree::PictureContent;

const MAX_NESTING: usize = 3;
const GZIP: [u8; 2] = [0x1F, 0x8B];

pub(crate) fn picture(href: &str, sources: &Sources<'_>) -> Option<PictureContent> {
    let href = href.trim();
    let (bytes, vector_hint) = match href.strip_prefix("data:") {
        Some(data) => {
            let (header, payload) = data.split_once(',')?;
            let bytes = match header.ends_with(";base64") {
                true => base64(payload)?,
                false => percent_decoded(payload),
            };
            (bytes, header.contains("svg"))
        }
        None => {
            if sources.depth > 0 {
                return None;
            }
            let plain = href.strip_prefix("file://").unwrap_or(href);
            let decoded = percent_decoded(plain);
            let path = std::str::from_utf8(&decoded).ok()?;
            let lower = path.to_ascii_lowercase();
            let vector = lower.ends_with(".svg") || lower.ends_with(".svgz");
            (linked_bytes(sources.folder?, path)?, vector)
        }
    };
    let markup = bytes
        .iter()
        .find(|byte| !byte.is_ascii_whitespace())
        .is_some_and(|byte| *byte == b'<');
    if vector_hint || markup || bytes.starts_with(&GZIP) {
        if sources.depth >= MAX_NESTING {
            return None;
        }
        let nested = Sources {
            folder: None,
            depth: sources.depth + 1,
            ..*sources
        };
        return parse_nested(&bytes, &nested)
            .ok()
            .map(|tree| PictureContent::Vector(Arc::new(tree)));
    }
    let bitmap = (sources.decoder?)(&bytes)?;
    if !sources.budget.spend(bitmap.rgba.len()) {
        return None;
    }
    Pixmap::from_straight(bitmap.width, bitmap.height, bitmap.rgba)
        .ok()
        .map(|pixmap| PictureContent::Raster(Arc::new(pixmap)))
}

fn base64(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len() * 3 / 4);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            byte if byte.is_ascii_whitespace() => continue,
            _ => return None,
        };
        buffer = buffer << 6 | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
        }
    }
    Some(bytes)
}

fn percent_decoded(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let escaped = (bytes[at] == b'%')
            .then(|| text.get(at + 1..at + 3))
            .flatten()
            .and_then(|digits| u8::from_str_radix(digits, 16).ok());
        match escaped {
            Some(byte) => {
                decoded.push(byte);
                at += 3;
            }
            None => {
                decoded.push(bytes[at]);
                at += 1;
            }
        }
    }
    decoded
}
