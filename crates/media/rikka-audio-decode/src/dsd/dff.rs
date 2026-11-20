use std::io::{Read, Seek};

use crate::dsd::layout::{Arrangement, Header, Span, read_at};

const CHUNK: u64 = 12;

#[derive(Default)]
struct Found {
    rate: Option<u32>,
    channels: Option<u16>,
    compressed: bool,
    sound: Option<(u64, u64)>,
    dst_frames: Option<(u32, u16)>,
    id3: Option<Span>,
}

pub(crate) fn header(source: &mut (impl Read + Seek)) -> Option<Header> {
    let form: [u8; 16] = read_at(source, 0)?;
    if &form[..4] != b"FRM8" || &form[12..16] != b"DSD " {
        return None;
    }
    let end = CHUNK.checked_add(u64_be(&form[4..12]))?;
    let mut found = Found::default();
    let mut at = 16u64;
    while at.saturating_add(CHUNK) <= end {
        let Some(chunk) = read_at::<12>(source, at) else {
            break;
        };
        let size = u64_be(&chunk[4..12]);
        let body = at + CHUNK;
        match &chunk[..4] {
            b"PROP" => properties(source, body, body.saturating_add(size), &mut found)?,
            b"DSD " => found.sound = Some((body, size)),
            b"DST " => {
                found.compressed = true;
                found.dst_frames = dst_frames(source, body, body.saturating_add(size));
            }
            id if id.eq_ignore_ascii_case(b"ID3 ") => {
                found.id3 = Some(Span {
                    at: body,
                    length: Some(size),
                });
            }
            _ => {}
        }
        let Some(next) = body
            .checked_add(size)
            .and_then(|next| next.checked_add(size % 2))
        else {
            break;
        };
        at = next;
    }
    let rate = found.rate.filter(|rate| *rate > 0)?;
    let channels = found.channels.filter(|channels| *channels > 0)?;
    let stream = found.sound.filter(|_| !found.compressed);
    let frames = match (stream, found.dst_frames) {
        (Some((_, bytes)), _) => bytes / u64::from(channels) * 8,
        (None, Some((count, per_second))) if per_second > 0 => {
            u64::from(count) * u64::from(rate) / u64::from(per_second)
        }
        (None, _) if found.compressed => 0,
        (None, _) => return None,
    };
    Some(Header {
        rate,
        channels,
        frames,
        stream: stream.map(|(data, _)| (data, Arrangement::Interleaved)),
        id3: found.id3,
    })
}

fn properties(
    source: &mut (impl Read + Seek),
    start: u64,
    end: u64,
    found: &mut Found,
) -> Option<()> {
    let kind: [u8; 4] = read_at(source, start)?;
    if &kind != b"SND " {
        return Some(());
    }
    let mut at = start + 4;
    while at.saturating_add(CHUNK) <= end {
        let chunk: [u8; 12] = read_at(source, at)?;
        let size = u64_be(&chunk[4..12]);
        let body = at + CHUNK;
        match &chunk[..4] {
            b"FS  " => found.rate = Some(u32::from_be_bytes(read_at(source, body)?)),
            b"CHNL" => found.channels = Some(u16::from_be_bytes(read_at(source, body)?)),
            b"CMPR" => found.compressed |= &read_at::<4>(source, body)? != b"DSD ",
            _ => {}
        }
        at = body.checked_add(size)?.checked_add(size % 2)?;
    }
    Some(())
}

fn dst_frames(source: &mut (impl Read + Seek), start: u64, end: u64) -> Option<(u32, u16)> {
    let mut at = start;
    while at.saturating_add(CHUNK) <= end {
        let chunk: [u8; 12] = read_at(source, at)?;
        let size = u64_be(&chunk[4..12]);
        let body = at + CHUNK;
        if &chunk[..4] == b"FRTE" {
            let pace: [u8; 6] = read_at(source, body)?;
            return Some((
                u32::from_be_bytes([pace[0], pace[1], pace[2], pace[3]]),
                u16::from_be_bytes([pace[4], pace[5]]),
            ));
        }
        at = body.checked_add(size)?.checked_add(size % 2)?;
    }
    None
}

fn u64_be(bytes: &[u8]) -> u64 {
    u64::from_be_bytes(bytes.try_into().unwrap_or_default())
}
