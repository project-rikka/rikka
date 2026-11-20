use crate::strip::{Malformed, exif};

pub(crate) const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const DROPPED: [&[u8; 4]; 5] = [b"eXIf", b"tEXt", b"zTXt", b"iTXt", b"tIME"];

pub(crate) fn strip(bytes: &[u8]) -> Result<Vec<u8>, Malformed> {
    let mut chunks = Vec::new();
    let mut orientation = None;
    let mut at = SIGNATURE.len();
    loop {
        let header = bytes.get(at..at + 8).ok_or(Malformed)?;
        let length = usize::try_from(u32::from_be_bytes([
            header[0], header[1], header[2], header[3],
        ]))
        .map_err(|_| Malformed)?;
        let kind: [u8; 4] = [header[4], header[5], header[6], header[7]];
        let chunk = bytes.get(at..at + 12 + length).ok_or(Malformed)?;
        at += chunk.len();
        if &kind == b"eXIf" {
            orientation = orientation.or_else(|| exif::orientation(&chunk[8..8 + length]));
        }
        if !DROPPED.contains(&&kind) {
            chunks.push((kind, chunk));
        }
        if &kind == b"IEND" {
            break;
        }
    }

    let mut out = Vec::with_capacity(bytes.len());
    out.extend_from_slice(SIGNATURE);
    let mut turned =
        orientation.map(|orientation| chunk(b"eXIf", &exif::only_orientation(orientation)));
    for (kind, bytes) in chunks {
        if &kind == b"IDAT"
            && let Some(turned) = turned.take()
        {
            out.extend_from_slice(&turned);
        }
        out.extend_from_slice(bytes);
    }
    Ok(out)
}

fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::with_capacity(data.len() + 12);
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(data);
    let mut crc = crc32fast::Hasher::new();
    crc.update(kind);
    crc.update(data);
    chunk.extend_from_slice(&crc.finalize().to_be_bytes());
    chunk
}
