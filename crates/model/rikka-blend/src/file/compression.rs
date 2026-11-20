use std::io::Read;

use rikka_model_core::{Error, Limit, Result};

use super::Source;

const GZIP: &[u8] = &[0x1f, 0x8b];
const ZSTD: &[u8] = &[0x28, 0xb5, 0x2f, 0xfd];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Packing {
    Plain,
    Gzip,
    Zstd,
}

pub(crate) fn packing(bytes: &[u8]) -> Packing {
    match () {
        () if bytes.starts_with(GZIP) => Packing::Gzip,
        () if bytes.starts_with(ZSTD) => Packing::Zstd,
        () => Packing::Plain,
    }
}

pub(crate) fn unpack(source: Source<'_>, limit: u64) -> Result<Source<'_>> {
    let bytes: &[u8] = &source;
    let packing = packing(bytes);
    let mut unpacked = Vec::new();
    let read = match packing {
        Packing::Plain => return Ok(source),
        Packing::Gzip => flate2::read::MultiGzDecoder::new(bytes)
            .take(limit.saturating_add(1))
            .read_to_end(&mut unpacked),
        Packing::Zstd => zstd::stream::read::Decoder::with_buffer(bytes).and_then(|decoder| {
            decoder
                .take(limit.saturating_add(1))
                .read_to_end(&mut unpacked)
        }),
    };
    read.map_err(|error| {
        Error::Damaged(format!(
            "failed to decompress {} .blend file: {error}",
            match packing {
                Packing::Gzip => "gzip",
                _ => "Zstandard",
            }
        ))
    })?;
    Limit::FileBytes.check(unpacked.len() as u64, limit)?;
    Ok(Source::Unpacked(unpacked))
}

pub(crate) fn peek(head: &[u8], length: usize) -> Option<Vec<u8>> {
    let mut peeked = vec![0; length];
    let read = match packing(head) {
        Packing::Plain => return head.get(..length).map(<[u8]>::to_vec),
        Packing::Gzip => flate2::read::GzDecoder::new(head).read_exact(&mut peeked),
        Packing::Zstd => zstd::stream::read::Decoder::with_buffer(head)
            .and_then(|mut decoder| decoder.read_exact(&mut peeked)),
    };
    read.ok().map(|()| peeked)
}
