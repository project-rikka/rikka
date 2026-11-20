use crate::strip::{Malformed, exif};

const EXTENDED: &[u8; 4] = b"VP8X";
const EXIF: &[u8; 4] = b"EXIF";
const XMP: &[u8; 4] = b"XMP ";
const EXIF_FLAG: u8 = 0x08;
const XMP_FLAG: u8 = 0x04;

pub(crate) fn is_webp(bytes: &[u8]) -> bool {
    bytes.get(..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
}

pub(crate) fn strip(bytes: &[u8]) -> Result<Vec<u8>, Malformed> {
    let declared = bytes
        .get(4..8)
        .map(|size| u32::from_le_bytes([size[0], size[1], size[2], size[3]]))
        .ok_or(Malformed)?;
    let end = usize::try_from(declared)
        .ok()
        .and_then(|size| size.checked_add(8))
        .filter(|end| *end <= bytes.len())
        .ok_or(Malformed)?;

    let mut out = Vec::with_capacity(end);
    out.extend_from_slice(b"RIFF\0\0\0\0WEBP");
    let mut flags_at = None;
    let mut kept_exif = false;
    let mut at = 12;
    while at + 8 <= end {
        let kind: [u8; 4] = bytes[at..at + 4].try_into().map_err(|_| Malformed)?;
        let size = usize::try_from(u32::from_le_bytes(
            bytes[at + 4..at + 8].try_into().map_err(|_| Malformed)?,
        ))
        .map_err(|_| Malformed)?;
        let padded = size + size % 2;
        let data = bytes.get(at + 8..at + 8 + size).ok_or(Malformed)?;
        let whole = bytes.get(at..(at + 8 + padded).min(end)).ok_or(Malformed)?;
        at += 8 + padded;
        match &kind {
            EXIF => {
                if let Some(orientation) = exif::orientation(data) {
                    push_chunk(&mut out, EXIF, &exif::only_orientation(orientation));
                    kept_exif = true;
                }
            }
            XMP => {}
            _ => {
                if &kind == EXTENDED {
                    flags_at = Some(out.len() + 8);
                }
                out.extend_from_slice(whole);
                if whole.len() % 2 == 1 {
                    out.push(0);
                }
            }
        }
    }

    if let Some(at) = flags_at {
        let flags = out.get_mut(at).ok_or(Malformed)?;
        *flags &= !(EXIF_FLAG | XMP_FLAG);
        if kept_exif {
            *flags |= EXIF_FLAG;
        }
    }
    let size = u32::try_from(out.len() - 8).map_err(|_| Malformed)?;
    out[4..8].copy_from_slice(&size.to_le_bytes());
    Ok(out)
}

fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(kind);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0);
    }
}
