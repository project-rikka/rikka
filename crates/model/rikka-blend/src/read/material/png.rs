use std::io::{self, Write};

use flate2::write::ZlibEncoder;
use flate2::{Compression, Crc};

const SIGNATURE: [u8; 8] = *b"\x89PNG\r\n\x1a\n";
const EIGHT_BITS: u8 = 8;
const TRUECOLOR_ALPHA: u8 = 6;
const UNFILTERED: u8 = 0;

pub(super) fn encode(width: u32, height: u32, rgba: &[u8]) -> io::Result<Vec<u8>> {
    let row = width as usize * 4;
    let mut scanlines = ZlibEncoder::new(Vec::new(), Compression::default());
    for pixels in rgba.chunks_exact(row.max(1)).take(height as usize) {
        scanlines.write_all(&[UNFILTERED])?;
        scanlines.write_all(pixels)?;
    }
    let packed = scanlines.finish()?;

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[EIGHT_BITS, TRUECOLOR_ALPHA, 0, 0, 0]);

    let mut png = Vec::with_capacity(SIGNATURE.len() + header.len() + packed.len() + 36);
    png.extend_from_slice(&SIGNATURE);
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &packed);
    chunk(&mut png, b"IEND", &[]);
    Ok(png)
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    let mut checksum = Crc::new();
    checksum.update(kind);
    checksum.update(data);
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    png.extend_from_slice(&checksum.sum().to_be_bytes());
}
