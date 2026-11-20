use rikka_deflate::Crc32;

use crate::error::{ChunkKind, Error, Result};

pub(crate) const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
pub(crate) const MAX_CHUNK: usize = 0x7FFF_FFFF;
pub(crate) const FRAME: usize = 12;

pub(crate) const IHDR: ChunkKind = ChunkKind(*b"IHDR");
pub(crate) const PLTE: ChunkKind = ChunkKind(*b"PLTE");
pub(crate) const IDAT: ChunkKind = ChunkKind(*b"IDAT");
pub(crate) const IEND: ChunkKind = ChunkKind(*b"IEND");
pub(crate) const TRNS: ChunkKind = ChunkKind(*b"tRNS");
pub(crate) const ICCP: ChunkKind = ChunkKind(*b"iCCP");
pub(crate) const EXIF: ChunkKind = ChunkKind(*b"eXIf");
pub(crate) const TEXT: ChunkKind = ChunkKind(*b"tEXt");
pub(crate) const ZTXT: ChunkKind = ChunkKind(*b"zTXt");
pub(crate) const ITXT: ChunkKind = ChunkKind(*b"iTXt");
pub(crate) const ACTL: ChunkKind = ChunkKind(*b"acTL");
pub(crate) const FCTL: ChunkKind = ChunkKind(*b"fcTL");
pub(crate) const FDAT: ChunkKind = ChunkKind(*b"fdAT");

#[derive(Clone, Copy, Debug)]
pub(crate) struct Chunk<'d> {
    pub(crate) kind: ChunkKind,
    pub(crate) body: &'d [u8],
}

pub(crate) struct Chunks<'d> {
    data: &'d [u8],
    at: usize,
}

impl<'d> Chunks<'d> {
    pub(crate) fn new(data: &'d [u8]) -> Result<Self> {
        match data.first_chunk::<8>() {
            Some(signature) if *signature == SIGNATURE => Ok(Self { data, at: 8 }),
            Some(_) => Err(Error::Signature),
            None if SIGNATURE.starts_with(data) && !data.is_empty() => Err(Error::Truncated),
            None => Err(Error::Signature),
        }
    }

    fn read(&mut self) -> Result<Chunk<'d>> {
        let rest = &self.data[self.at..];
        let (length, rest) = rest.split_first_chunk::<4>().ok_or(Error::Truncated)?;
        let (kind, rest) = rest.split_first_chunk::<4>().ok_or(Error::Truncated)?;
        let kind = ChunkKind(*kind);
        let length = u32::from_be_bytes(*length) as usize;
        if length > MAX_CHUNK {
            return Err(Error::Chunk(kind));
        }
        let body = rest.get(..length).ok_or(Error::Truncated)?;
        let stored = rest[length..].first_chunk::<4>().ok_or(Error::Truncated)?;
        self.at += FRAME + length;
        let mut crc = Crc32::new();
        crc.update(&kind.0);
        crc.update(body);
        if crc.finish() != u32::from_be_bytes(*stored) {
            return Err(Error::Checksum(kind));
        }
        Ok(Chunk { kind, body })
    }
}

impl<'d> Iterator for Chunks<'d> {
    type Item = Result<Chunk<'d>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.at >= self.data.len() {
            return None;
        }
        let chunk = self.read();
        if matches!(chunk, Err(Error::Truncated | Error::Chunk(_))) {
            self.at = self.data.len();
        }
        Some(chunk)
    }
}

pub(crate) fn write(out: &mut Vec<u8>, kind: ChunkKind, body: &[u8]) -> Result<()> {
    let length = u32::try_from(body.len())
        .ok()
        .filter(|&length| length as usize <= MAX_CHUNK)
        .ok_or(Error::TooLarge)?;
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(&kind.0);
    out.extend_from_slice(body);
    let mut crc = Crc32::new();
    crc.update(&kind.0);
    crc.update(body);
    out.extend_from_slice(&crc.finish().to_be_bytes());
    Ok(())
}
