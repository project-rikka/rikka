use std::io::{ErrorKind, Read, Seek, SeekFrom};

use rikka_deflate::{Crc32, Inflater};

use crate::chunk::{ACTL, EXIF, IDAT, IEND, IHDR, ITXT, MAX_CHUNK, SIGNATURE, TEXT, TRNS, ZTXT};
use crate::error::{ChunkKind, Error, Result};
use crate::header::{Color, Header};
use crate::layout::Layout;
use crate::text::{self, MAX_TEXT, Text};

const MAX_EXIF: usize = 1 << 20;
const MAX_TEXT_TOTAL: usize = 16 << 20;
const CHECKSUM: i64 = 4;
const ANIMATION_CONTROL_BYTES: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub width: u32,
    pub height: u32,
    pub layout: Layout,
    pub animated: bool,
    pub exif: Option<Vec<u8>>,
    pub text: Vec<Text>,
}

pub fn probe<R: Read + Seek>(mut reader: R) -> Result<Info> {
    let mut signature = [0u8; 8];
    match reader.read_exact(&mut signature) {
        Ok(()) if signature == SIGNATURE => {}
        Ok(()) => return Err(Error::Signature),
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => return Err(Error::Signature),
        Err(error) => return Err(Error::Io(error)),
    }
    let mut header: Option<Header> = None;
    let mut keyed = false;
    let mut animated = false;
    let mut image_seen = false;
    let mut exif = None;
    let mut entries = Vec::new();
    let mut text_budget = MAX_TEXT_TOTAL;
    let mut inflater = Inflater::new();
    let mut body = Vec::new();
    loop {
        let mut frame = [0u8; 8];
        match reader.read_exact(&mut frame) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(Error::Io(error)),
        }
        let length = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        let kind = ChunkKind([frame[4], frame[5], frame[6], frame[7]]);
        if length > MAX_CHUNK {
            return Err(Error::Chunk(kind));
        }
        if header.is_none() && kind != IHDR {
            return Err(Error::Header);
        }
        let wanted = match kind {
            IHDR if length != Header::LEN => return Err(Error::Header),
            ACTL if length != ANIMATION_CONTROL_BYTES => return Err(Error::Chunk(ACTL)),
            IHDR | ACTL => true,
            EXIF => length <= MAX_EXIF,
            TEXT | ZTXT | ITXT => length <= MAX_TEXT.min(text_budget),
            _ => false,
        };
        if !wanted {
            match kind {
                IEND => break,
                IDAT => image_seen = true,
                TRNS => keyed = true,
                _ => {}
            }
            reader.seek(SeekFrom::Current(length as i64 + CHECKSUM))?;
            continue;
        }
        body.clear();
        body.resize(length + CHECKSUM as usize, 0);
        match reader.read_exact(&mut body) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::UnexpectedEof => break,
            Err(error) => return Err(Error::Io(error)),
        }
        let (content, stored) = body.split_at(length);
        let mut crc = Crc32::new();
        crc.update(&kind.0);
        crc.update(content);
        if stored != crc.finish().to_be_bytes() {
            if kind.is_critical() {
                return Err(Error::Checksum(kind));
            }
            continue;
        }
        match kind {
            IHDR if header.is_none() => header = Some(Header::parse(content)?),
            IHDR => return Err(Error::Chunk(IHDR)),
            ACTL => animated |= !image_seen,
            EXIF => exif = exif.or_else(|| Some(content.to_vec())),
            _ => {
                text_budget -= length;
                entries.extend(text::decode(kind, content, &mut inflater));
            }
        }
    }
    let header = header.ok_or(Error::Truncated)?;
    let keyed = keyed && matches!(header.color, Color::Gray | Color::Rgb | Color::Indexed);
    Ok(Info {
        width: header.width as u32,
        height: header.height as u32,
        layout: header.layout(keyed),
        animated,
        exif,
        text: entries,
    })
}
