use crate::codecs::{Limits, Probe, Reader, bmp, damaged, png};
use crate::color_type::ColorType;
use crate::error::{Error, Result};
use crate::format::Format;
use crate::raster::Image;
use crate::write::Compression;

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const ICON: u16 = 1;
const CURSOR: u16 = 2;
const ENTRY: usize = 16;
const DIRECTORY: usize = 6;
const LARGEST_SIDE: u32 = 256;

fn best_entry(data: &[u8]) -> Result<&[u8]> {
    let mut reader = Reader::new(Format::Ico, data);
    let reserved = reader.u16_le()?;
    let kind = reader.u16_le()?;
    let count = reader.u16_le()?;
    if reserved != 0 || !matches!(kind, ICON | CURSOR) || count == 0 {
        return Err(damaged(Format::Ico, "invalid directory"));
    }
    let mut best: Option<(u32, u16, &[u8])> = None;
    for _ in 0..count {
        let side = |raw: u8| match raw {
            0 => LARGEST_SIDE,
            side => u32::from(side),
        };
        let area = side(reader.u8()?) * side(reader.u8()?);
        reader.skip(4)?;
        let depth = match kind {
            ICON => reader.u16_le()?,
            _ => reader.u16_le().map(|_| 0)?,
        };
        let size = reader.u32_le()? as usize;
        let offset = reader.u32_le()? as usize;
        let Some(content) = data.get(offset..).and_then(|rest| rest.get(..size)) else {
            continue;
        };
        if best.is_none_or(|(most_area, most_depth, _)| (area, depth) > (most_area, most_depth)) {
            best = Some((area, depth, content));
        }
    }
    best.map(|(_, _, content)| content)
        .ok_or_else(|| damaged(Format::Ico, "no readable entry"))
}

pub(crate) fn probe(data: &[u8]) -> Result<Probe> {
    let content = best_entry(data)?;
    let inner = match content.starts_with(PNG_SIGNATURE) {
        true => png::probe(std::io::Cursor::new(content))?.probe,
        false => bmp::probe_icon(content)?,
    };
    Ok(Probe {
        color: ColorType::Rgba8,
        ..inner
    })
}

pub(crate) fn decode(data: &[u8], limits: &Limits) -> Result<Image> {
    let content = best_entry(data)?;
    let image = match content.starts_with(PNG_SIGNATURE) {
        true => png::decode(content, limits)?.0,
        false => bmp::decode_icon(content, limits)?,
    };
    Ok(image.convert(ColorType::Rgba8))
}

pub(crate) fn encode(image: &Image, out: &mut Vec<u8>) -> Result<()> {
    let (width, height) = image.dimensions();
    if width > LARGEST_SIDE || height > LARGEST_SIDE {
        return Err(Error::Encoding(format!(
            "ICO: {width}x{height} exceeds 256 pixels a side"
        )));
    }
    let mut content = Vec::new();
    png::encode(image, Compression::Balanced, None, &[], &mut content)?;
    out.extend_from_slice(&[0, 0]);
    out.extend_from_slice(&ICON.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&[width as u8, height as u8, 0, 0]);
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&u16::from(image.color().bytes_per_pixel() * 8).to_le_bytes());
    out.extend_from_slice(&(content.len() as u32).to_le_bytes());
    out.extend_from_slice(&((DIRECTORY + ENTRY) as u32).to_le_bytes());
    out.extend_from_slice(&content);
    Ok(())
}
