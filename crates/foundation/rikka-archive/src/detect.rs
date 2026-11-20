use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::error::{Error, Result};
use crate::format::Format;
use crate::stream::StreamCompression;

const TAR_BLOCK: usize = 512;

const HEAD_PROBE_LEN: u64 = 1024;

const PROBE_STEP: u64 = 256 * 1024;
const PROBE_LIMIT: usize = 4 * 1024 * 1024;

pub fn detect(path: impl AsRef<Path>) -> Result<Format> {
    let path = path.as_ref();
    let mut file = File::open(path)?;

    detect_source(&mut file).or_else(|error| Format::from_path(path).ok_or(error))
}

pub(crate) fn detect_reader<R: Read + Seek>(source: &mut R) -> Result<Format> {
    let start = source.stream_position()?;
    let format = detect_source(source);
    source.seek(SeekFrom::Start(start))?;
    format
}

fn detect_source<R: Read>(source: &mut R) -> Result<Format> {
    let mut probe = Vec::new();
    source.take(HEAD_PROBE_LEN).read_to_end(&mut probe)?;

    let surface = surface_format(&probe).ok_or(Error::UnknownFormat)?;
    let Some(compression) = surface.stream_compression() else {
        return Ok(surface);
    };

    loop {
        if let Some(wraps_tar) = decode_head(compression, &probe) {
            return Ok(if wraps_tar {
                surface.into_tar()
            } else {
                surface
            });
        }

        let before = probe.len();
        if before >= PROBE_LIMIT {
            return Ok(surface);
        }
        source.take(PROBE_STEP).read_to_end(&mut probe)?;
        if probe.len() == before {
            return Ok(surface);
        }
    }
}

fn surface_format(data: &[u8]) -> Option<Format> {
    if data.starts_with(b"PK\x03\x04") || data.starts_with(b"PK\x05\x06") {
        return Some(Format::Zip);
    }
    if data.starts_with(&[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]) {
        return Some(Format::SevenZ);
    }

    for (compression, format) in [
        (StreamCompression::Gzip, Format::Gzip),
        (StreamCompression::Bzip2, Format::Bzip2),
        (StreamCompression::Xz, Format::Xz),
        (StreamCompression::Zstd, Format::Zstd),
    ] {
        if data.starts_with(compression.magic()) {
            return Some(format);
        }
    }

    (is_tar_header(data) || is_empty_tar(data)).then_some(Format::Tar)
}

fn decode_head(compression: StreamCompression, compressed: &[u8]) -> Option<bool> {
    let Ok(decoder) = compression.decoder(compressed) else {
        return Some(false);
    };

    let mut block = Vec::with_capacity(TAR_BLOCK);
    if decoder
        .take(TAR_BLOCK as u64)
        .read_to_end(&mut block)
        .is_err()
        || block.len() < TAR_BLOCK
    {
        return None;
    }

    Some(is_tar_header(&block) || is_empty_tar(&block))
}

fn is_tar_header(block: &[u8]) -> bool {
    if block.len() < TAR_BLOCK {
        return false;
    }
    if block[257..262] == *b"ustar" {
        return true;
    }

    let Some(stored) = parse_octal(&block[148..156]) else {
        return false;
    };
    let (unsigned, signed) = header_checksums(block);
    stored == u64::from(unsigned) || stored == signed.unsigned_abs().into()
}

fn is_empty_tar(block: &[u8]) -> bool {
    block.len() >= TAR_BLOCK && block[..TAR_BLOCK].iter().all(|&byte| byte == 0)
}

fn header_checksums(block: &[u8]) -> (u32, i32) {
    let mut unsigned = 0u32;
    let mut signed = 0i32;
    for (index, &byte) in block[..TAR_BLOCK].iter().enumerate() {
        let byte = if (148..156).contains(&index) {
            b' '
        } else {
            byte
        };
        unsigned += u32::from(byte);
        signed += i32::from(byte as i8);
    }
    (unsigned, signed)
}

fn parse_octal(field: &[u8]) -> Option<u64> {
    let text = std::str::from_utf8(field).ok()?;
    let digits = text.trim_matches(|c: char| c == '\0' || c == ' ');
    if digits.is_empty() {
        return None;
    }
    u64::from_str_radix(digits, 8).ok()
}
