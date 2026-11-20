use std::path::Path;
use std::time::Duration;

use crate::error::{Error, Result};

const MAGIC: &[u8; 4] = b"Xcur";

const IMAGE: u32 = 0xfffd_0002;

const LARGEST_SIDE: u32 = 0x7fff;

const TOC_ENTRY: usize = 12;

const IMAGE_HEADER: usize = 36;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub hotspot: (u32, u32),
    pub delay: Duration,
    pub bgra: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub size: u32,
    pub frames: Vec<Frame>,
}

impl Cursor {
    pub fn read(path: &Path, size: u32) -> Result<Cursor> {
        Cursor::parse(&std::fs::read(path)?, size)
    }

    pub fn parse(file: &[u8], size: u32) -> Result<Cursor> {
        if file.get(..MAGIC.len()) != Some(MAGIC) {
            return Err(Error::NotXcursor);
        }
        let header = word(file, 4, "its header")? as usize;
        let entries = word(file, 12, "its header")? as usize;
        let images: Vec<(u32, usize)> = (0..entries)
            .map(|entry| {
                let at = header.saturating_add(entry.saturating_mul(TOC_ENTRY));
                let kind = word(file, at, "its table of contents")?;
                let nominal = word(file, at.saturating_add(4), "its table of contents")?;
                let position = word(file, at.saturating_add(8), "its table of contents")?;
                Ok((kind, nominal, position as usize))
            })
            .filter_map(|entry: Result<(u32, u32, usize)>| match entry {
                Ok((IMAGE, nominal, position)) => Some(Ok((nominal, position))),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<_>>()?;
        let nearest = images
            .iter()
            .map(|(nominal, _)| *nominal)
            .min_by_key(|nominal| nominal.abs_diff(size))
            .ok_or(Error::NoImage)?;
        let frames = images
            .iter()
            .filter(|(nominal, _)| *nominal == nearest)
            .map(|(_, position)| frame(file, *position))
            .collect::<Result<_>>()?;
        Ok(Cursor {
            size: nearest,
            frames,
        })
    }
}

fn frame(file: &[u8], at: usize) -> Result<Frame> {
    let field = |index: usize| word(file, at.saturating_add(index * 4), "an image header");
    let (width, height) = (field(4)?, field(5)?);
    if width > LARGEST_SIDE || height > LARGEST_SIDE {
        return Err(Error::Oversized { width, height });
    }
    let pixels = at.saturating_add(IMAGE_HEADER);
    let length = width as usize * height as usize * 4;
    let bgra = file
        .get(pixels..pixels.saturating_add(length))
        .ok_or(Error::Truncated("an image"))?
        .to_vec();
    Ok(Frame {
        width,
        height,
        hotspot: (field(6)?, field(7)?),
        delay: Duration::from_millis(u64::from(field(8)?)),
        bgra,
    })
}

fn word(file: &[u8], at: usize, inside: &'static str) -> Result<u32> {
    file.get(at..at.saturating_add(4))
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(Error::Truncated(inside))
}
