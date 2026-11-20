use crate::codecs::{Limits, Probe, Reader, damaged};
use crate::color_type::ColorType;
use crate::error::Result;
use crate::format::Format;
use crate::pixels::Pixels;
use crate::raster::Image;

const MAGIC: &[u8; 8] = b"farbfeld";

fn header(reader: &mut Reader<'_>) -> Result<Probe> {
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(damaged(Format::Farbfeld, "missing signature"));
    }
    Ok(Probe {
        width: reader.u32_be()?,
        height: reader.u32_be()?,
        color: ColorType::Rgba16,
    })
}

pub(crate) fn probe(data: &[u8]) -> Result<Probe> {
    header(&mut Reader::new(Format::Farbfeld, data))
}

pub(crate) fn decode(data: &[u8], limits: &Limits) -> Result<Image> {
    let mut reader = Reader::new(Format::Farbfeld, data);
    let Probe {
        width,
        height,
        color,
    } = header(&mut reader)?;
    let bytes = limits.admit(width, height, color)?;
    let samples = reader
        .take(bytes)?
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect();
    Ok(Image::from_pixels(
        width,
        height,
        color,
        Pixels::U16(samples),
    ))
}

pub(crate) fn encode(image: &Image, out: &mut Vec<u8>) {
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&image.width().to_be_bytes());
    out.extend_from_slice(&image.height().to_be_bytes());
    let Pixels::U16(samples) = &image.pixels else {
        unreachable!("converted to 16-bit RGBA before encoding");
    };
    out.reserve(samples.len() * 2);
    for sample in samples {
        out.extend_from_slice(&sample.to_be_bytes());
    }
}
