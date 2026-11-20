use std::io::{Read, Seek, SeekFrom};

use crate::encoding::{Codec, Encoding};

const HEAD: usize = 64;
const ASF: &[u8] = b"\x30\x26\xB2\x75\x8E\x66\xCF\x11";
const WAVPACK_HYBRID: u32 = 0x8;

pub(crate) fn foreign(source: &mut (impl Read + Seek)) -> std::io::Result<Option<Encoding>> {
    let head = head(source);
    source.seek(SeekFrom::Start(0))?;
    Ok(head.as_deref().and_then(classify))
}

fn head(source: &mut (impl Read + Seek)) -> Option<Vec<u8>> {
    let mut start = 0u64;
    let mut head = read_at(source, start)?;
    if let Some(skip) = id3v2_length(&head) {
        start += skip;
        head = read_at(source, start)?;
    }
    Some(head)
}

fn read_at(source: &mut (impl Read + Seek), at: u64) -> Option<Vec<u8>> {
    source.seek(SeekFrom::Start(at)).ok()?;
    let mut head = Vec::with_capacity(HEAD);
    source.take(HEAD as u64).read_to_end(&mut head).ok()?;
    Some(head)
}

fn id3v2_length(head: &[u8]) -> Option<u64> {
    if head.len() < 10 || &head[..3] != b"ID3" {
        return None;
    }
    let size = head[6..10]
        .iter()
        .fold(0u64, |size, byte| (size << 7) | u64::from(byte & 0x7f));
    let footer = if head[5] & 0x10 != 0 { 10 } else { 0 };
    Some(10 + size + footer)
}

fn classify(head: &[u8]) -> Option<Encoding> {
    let at = |offset: usize, bytes: &[u8]| head.get(offset..offset + bytes.len()) == Some(bytes);
    let codec = if at(0, b"wvpk") {
        let flags = head.get(24..28).map_or(0, |flags| {
            u32::from_le_bytes([flags[0], flags[1], flags[2], flags[3]])
        });
        let mut encoding = Encoding::for_codec(Codec::WavPack);
        encoding.lossless = flags & WAVPACK_HYBRID == 0;
        return Some(encoding);
    } else if at(0, b"MAC ") {
        Codec::Ape
    } else if at(0, b"tBaK") {
        Codec::Tak
    } else if at(0, b"MPCK") || at(0, b"MP+") {
        Codec::Musepack
    } else if at(0, b"DSD ") || (at(0, b"FRM8") && at(12, b"DSD ")) {
        Codec::Dsd
    } else if at(0, ASF) {
        Codec::Wma
    } else if at(0, b"\x0B\x77") {
        match head.get(5).map(|byte| byte >> 3) {
            Some(bsid) if bsid > 10 => Codec::Eac3,
            _ => Codec::Ac3,
        }
    } else {
        return None;
    };
    Some(Encoding::for_codec(codec))
}
