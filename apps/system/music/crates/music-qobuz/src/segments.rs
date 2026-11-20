use std::io;

use music_core::{Error, Result};

const INIT_UUID: [u8; 16] = [
    0xc7, 0xc7, 0x5d, 0xf0, 0xfd, 0xd9, 0x51, 0xe9, 0x8f, 0xc2, 0x29, 0x71, 0xe4, 0xac, 0xf8, 0xd2,
];
const SEGMENT_UUID: [u8; 16] = [
    0x3b, 0x42, 0x12, 0x92, 0x56, 0xf3, 0x5f, 0x75, 0x92, 0x36, 0x63, 0xb6, 0x9a, 0x1f, 0x52, 0xb2,
];
const FLAC: &[u8; 4] = b"fLaC";
const STREAM_INFO: usize = 4 + 4 + 34;
const LAST_BLOCK: u8 = 0x80;
const HEADER: u64 = 8;
const LARGE_HEADER: u64 = 16;
const UUID_PAYLOAD: usize = 8 + 16;
const INIT_FIXED: usize = 4 + 4 + 4 + 4 + 1 + 3 + 6;
const IV: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Init {
    pub(crate) header: Vec<u8>,
    pub(crate) lengths: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Piece {
    pub(crate) at: u64,
    pub(crate) len: u64,
    pub(crate) iv: Option<[u8; IV]>,
}

pub(crate) fn init(bytes: &[u8]) -> Result<Init> {
    let payload = find_uuid(bytes, &INIT_UUID)
        .ok_or_else(|| unreadable("the init segment has no track description"))?;
    let mut cursor = Cursor::new(payload, INIT_FIXED);
    let raw = cursor.take_sized(2)?;
    let start = raw
        .windows(FLAC.len())
        .position(|window| window == FLAC)
        .ok_or_else(|| unreadable("the track is not FLAC"))?;
    let mut header = raw
        .get(start..start + STREAM_INFO)
        .ok_or_else(|| unreadable("the FLAC stream info is cut short"))?
        .to_vec();
    header[FLAC.len()] |= LAST_BLOCK;
    cursor.take_sized(1)?;
    let count = cursor.number(2)?;
    let lengths = (0..count)
        .map(|_| {
            let length = cursor.number(4)?;
            cursor.number(4)?;
            Ok(length)
        })
        .collect::<Result<Vec<u64>>>()?;
    if lengths.is_empty() {
        return Err(unreadable("the track has no segments"));
    }
    Ok(Init { header, lengths })
}

pub(crate) fn pieces(
    record: u64,
    read: &mut impl FnMut(u64, &mut [u8]) -> io::Result<()>,
) -> Result<Vec<Piece>> {
    let mut at = 0;
    let mut crypto: Option<(u64, Vec<u8>)> = None;
    let mut media: Option<u64> = None;
    while at + HEADER <= record && (crypto.is_none() || media.is_none()) {
        let mut head = [0; 16];
        read(at, &mut head[..HEADER as usize]).map_err(stored)?;
        let kind = [head[4], head[5], head[6], head[7]];
        let size = match u32::from_be_bytes([head[0], head[1], head[2], head[3]]) {
            0 => record - at,
            1 => {
                read(
                    at + HEADER,
                    &mut head[HEADER as usize..LARGE_HEADER as usize],
                )
                .map_err(stored)?;
                u64::from_be_bytes([
                    head[8], head[9], head[10], head[11], head[12], head[13], head[14], head[15],
                ])
            }
            size => u64::from(size),
        };
        if size < HEADER || at + size > record {
            return Err(unreadable("a segment box runs past its segment"));
        }
        match &kind {
            b"uuid" => {
                let mut whole = vec![0; usize::try_from(size).map_err(|_| too_large())?];
                read(at, &mut whole).map_err(stored)?;
                if whole.get(8..24) == Some(&SEGMENT_UUID[..]) {
                    crypto = Some((at, whole));
                }
            }
            b"mdat" => media = Some(at + size),
            _ => {}
        }
        at += size;
    }
    let (Some((start, whole)), Some(end)) = (crypto, media) else {
        return Err(unreadable("a segment has no encrypted audio"));
    };
    laid_out(start, &whole, end)
}

fn laid_out(start: u64, whole: &[u8], end: u64) -> Result<Vec<Piece>> {
    let mut cursor = Cursor::new(whole, UUID_PAYLOAD + 4);
    let data = start + cursor.number(4)?;
    let iv = usize::try_from(cursor.number(1)?).map_err(|_| too_large())?;
    let frames = cursor.number(3)?;
    let mut pieces = Vec::new();
    let mut at = data;
    for _ in 0..frames {
        let len = cursor.number(4)?;
        cursor.skip(2)?;
        let encrypted = cursor.number(2)? != 0;
        let mut vector = [0; IV];
        let iv_bytes = cursor.take(iv)?;
        let copied = iv_bytes.len().min(IV);
        vector[..copied].copy_from_slice(&iv_bytes[..copied]);
        pieces.push(Piece {
            at,
            len,
            iv: encrypted.then_some(vector),
        });
        at += len;
    }
    if at > end {
        return Err(unreadable("a segment's frames run past its audio"));
    }
    if at < end {
        pieces.push(Piece {
            at,
            len: end - at,
            iv: None,
        });
    }
    Ok(pieces)
}

fn find_uuid<'a>(bytes: &'a [u8], uuid: &[u8; 16]) -> Option<&'a [u8]> {
    let mut at = 0;
    while at + HEADER as usize <= bytes.len() {
        let size = u32::from_be_bytes(bytes[at..at + 4].try_into().ok()?) as usize;
        let size = match size {
            0 => bytes.len() - at,
            size if size < HEADER as usize => return None,
            size => size,
        };
        let whole = bytes.get(at..at + size)?;
        if &whole[4..8] == b"uuid" && whole.get(8..24) == Some(&uuid[..]) {
            return whole.get(UUID_PAYLOAD..);
        }
        at += size;
    }
    None
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], at: usize) -> Cursor<'a> {
        Cursor { bytes, at }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        let read = self
            .bytes
            .get(self.at..self.at + count)
            .ok_or_else(|| unreadable("a segment description is cut short"))?;
        self.at += count;
        Ok(read)
    }

    fn skip(&mut self, count: usize) -> Result<()> {
        self.take(count).map(drop)
    }

    fn number(&mut self, width: usize) -> Result<u64> {
        Ok(self
            .take(width)?
            .iter()
            .fold(0, |number, byte| number << 8 | u64::from(*byte)))
    }

    fn take_sized(&mut self, width: usize) -> Result<&'a [u8]> {
        let count = usize::try_from(self.number(width)?).map_err(|_| too_large())?;
        self.take(count)
    }
}

fn stored(error: io::Error) -> Error {
    Error::Unreadable(format!("cannot read the cached track: {error}"))
}

fn too_large() -> Error {
    unreadable("a segment is larger than this computer can hold")
}

fn unreadable(message: &str) -> Error {
    Error::Unreadable(message.to_owned())
}
