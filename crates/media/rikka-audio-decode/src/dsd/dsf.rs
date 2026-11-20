use std::io::{Read, Seek};

use crate::dsd::layout::{Arrangement, Header, Span, read_at};

const HEADER: u64 = 28;
const RAW_DSD: u32 = 0;

pub(crate) fn header(source: &mut (impl Read + Seek)) -> Option<Header> {
    let header: [u8; 28] = read_at(source, 0)?;
    if &header[..4] != b"DSD " {
        return None;
    }
    let metadata = u64_le(&header[20..28]);
    let format: [u8; 52] = read_at(source, HEADER)?;
    if &format[..4] != b"fmt " {
        return None;
    }
    let format_size = u64_le(&format[4..12]);
    if u32_le(&format[16..20]) != RAW_DSD {
        return None;
    }
    let channels = u16::try_from(u32_le(&format[24..28])).ok()?;
    let rate = u32_le(&format[28..32]);
    let lsb_first = match u32_le(&format[32..36]) {
        1 => true,
        8 => false,
        _ => return None,
    };
    let frames = u64_le(&format[36..44]);
    let block = u64::from(u32_le(&format[44..48]));
    let data_at = HEADER.checked_add(format_size)?;
    let data: [u8; 12] = read_at(source, data_at)?;
    if &data[..4] != b"data" || channels == 0 || rate == 0 || block == 0 {
        return None;
    }
    Some(Header {
        rate,
        channels,
        frames,
        stream: Some((data_at + 12, Arrangement::Blocked { block, lsb_first })),
        id3: (metadata > 0).then_some(Span {
            at: metadata,
            length: None,
        }),
    })
}

fn u32_le(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes.try_into().unwrap_or_default())
}

fn u64_le(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().unwrap_or_default())
}
