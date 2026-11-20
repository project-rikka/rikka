use crate::strip::{Malformed, exif};

const MARK: u8 = 0xFF;
const START: u8 = 0xD8;
const END: u8 = 0xD9;
const SCAN: u8 = 0xDA;
const APP0: u8 = 0xE0;
const APP1: u8 = 0xE1;
const APP2: u8 = 0xE2;
const APP14: u8 = 0xEE;
const COMMENT: u8 = 0xFE;
const RESTARTS: std::ops::RangeInclusive<u8> = 0xD0..=0xD7;
const STUFFED_BYTE: u8 = 0x00;

pub(crate) fn strip(bytes: &[u8]) -> Result<Vec<u8>, Malformed> {
    if bytes.get(..2) != Some(&[MARK, START]) {
        return Err(Malformed);
    }
    let mut leading = Vec::new();
    let mut body = Vec::with_capacity(bytes.len());
    let mut orientation = None;
    let mut at = 2;
    loop {
        while bytes.get(at) == Some(&MARK) && bytes.get(at + 1) == Some(&MARK) {
            at += 1;
        }
        let (Some(&MARK), Some(&marker)) = (bytes.get(at), bytes.get(at + 1)) else {
            break;
        };
        if marker == END {
            break;
        }
        if RESTARTS.contains(&marker) || marker == 0x01 {
            body.extend_from_slice(&bytes[at..at + 2]);
            at += 2;
            continue;
        }
        let length = bytes
            .get(at + 2..at + 4)
            .map(|length| usize::from(u16::from_be_bytes([length[0], length[1]])))
            .filter(|length| *length >= 2)
            .ok_or(Malformed)?;
        let segment = bytes.get(at..at + 2 + length).ok_or(Malformed)?;
        let payload = &segment[4..];
        at += segment.len();
        match marker {
            APP0 if payload.starts_with(b"JFIF\0") && leading.is_empty() => {
                leading.extend_from_slice(segment);
            }
            APP1 if payload.starts_with(exif::JPEG_PREFIX) => {
                orientation = orientation.or_else(|| exif::orientation(payload));
            }
            APP2 if payload.starts_with(b"ICC_PROFILE\0") => body.extend_from_slice(segment),
            APP14 if payload.starts_with(b"Adobe") => body.extend_from_slice(segment),
            APP0..=APP14 | 0xEF | COMMENT => {}
            SCAN => {
                body.extend_from_slice(segment);
                let ends = entropy_end(bytes, at);
                body.extend_from_slice(&bytes[at..ends]);
                at = ends;
            }
            _ => body.extend_from_slice(segment),
        }
    }

    let mut out = Vec::with_capacity(body.len() + leading.len() + 64);
    out.extend_from_slice(&[MARK, START]);
    out.extend_from_slice(&leading);
    if let Some(orientation) = orientation {
        let mut payload = exif::JPEG_PREFIX.to_vec();
        payload.extend_from_slice(&exif::only_orientation(orientation));
        let length = u16::try_from(payload.len() + 2).map_err(|_| Malformed)?;
        out.extend_from_slice(&[MARK, APP1]);
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(&payload);
    }
    out.extend_from_slice(&body);
    out.extend_from_slice(&[MARK, END]);
    Ok(out)
}

fn entropy_end(bytes: &[u8], from: usize) -> usize {
    let mut at = from;
    while let Some(offset) = bytes[at..].iter().position(|byte| *byte == MARK) {
        at += offset;
        match bytes.get(at + 1) {
            Some(&STUFFED_BYTE) => at += 2,
            Some(next) if RESTARTS.contains(next) => at += 2,
            Some(&MARK) => at += 1,
            Some(_) => return at,
            None => return bytes.len(),
        }
    }
    bytes.len()
}
