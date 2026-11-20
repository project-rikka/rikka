use crate::codecs::{Limits, Probe, Reader, damaged, plausible, unsupported};
use crate::color_type::ColorType;
use crate::error::{Error, Result};
use crate::format::Format;
use crate::pixels::Pixels;
use crate::raster::Image;

const COLOR_MAPPED: u8 = 1;
const TRUE_COLOR: u8 = 2;
const GRAY: u8 = 3;
const RUN_LENGTH: u8 = 8;
const ALPHA_BITS: u8 = 0x0F;
const RIGHT_TO_LEFT: u8 = 0x10;
const TOP_TO_BOTTOM: u8 = 0x20;
const RUN_PACKET: u8 = 0x80;
const LONGEST_PACKET: usize = 128;
const MOST_EXPANSION: usize = 1024;

#[derive(Clone, Copy, Debug)]
struct Header {
    id_length: usize,
    kind: u8,
    compressed: bool,
    map_first: usize,
    map_length: usize,
    map_entry_bits: u8,
    width: u32,
    height: u32,
    bits: u8,
    descriptor: u8,
}

impl Header {
    fn read(reader: &mut Reader<'_>) -> Result<Header> {
        let id_length = usize::from(reader.u8()?);
        let map_kind = reader.u8()?;
        let image_kind = reader.u8()?;
        let map_first = usize::from(reader.u16_le()?);
        let map_length = usize::from(reader.u16_le()?);
        let map_entry_bits = reader.u8()?;
        reader.skip(4)?;
        let width = u32::from(reader.u16_le()?);
        let height = u32::from(reader.u16_le()?);
        let bits = reader.u8()?;
        let descriptor = reader.u8()?;
        let kind = image_kind & !RUN_LENGTH;
        let valid = map_kind <= 1
            && matches!(kind, COLOR_MAPPED | TRUE_COLOR | GRAY)
            && image_kind & !(RUN_LENGTH | 3) == 0
            && (kind != COLOR_MAPPED || (map_kind == 1 && map_length > 0))
            && descriptor & !(ALPHA_BITS | RIGHT_TO_LEFT | TOP_TO_BOTTOM) == 0;
        if !valid {
            return Err(damaged(Format::Tga, "invalid header"));
        }
        let header = Header {
            id_length,
            kind,
            compressed: image_kind & RUN_LENGTH != 0,
            map_first,
            map_length,
            map_entry_bits,
            width,
            height,
            bits,
            descriptor,
        };
        header.color()?;
        Ok(header)
    }

    fn color(&self) -> Result<ColorType> {
        let texel_bits = match self.kind {
            COLOR_MAPPED if matches!(self.bits, 8 | 16) => self.map_entry_bits,
            COLOR_MAPPED => 0,
            _ => self.bits,
        };
        match (self.kind, texel_bits) {
            (GRAY, 8) => Ok(ColorType::L8),
            (GRAY, 16) => Ok(ColorType::La8),
            (TRUE_COLOR | COLOR_MAPPED, 15 | 16 | 24) => Ok(ColorType::Rgb8),
            (TRUE_COLOR | COLOR_MAPPED, 32) => Ok(ColorType::Rgba8),
            _ => Err(unsupported(Format::Tga, "pixel depth")),
        }
    }

    fn probe(&self) -> Result<Probe> {
        Ok(Probe {
            width: self.width,
            height: self.height,
            color: self.color()?,
        })
    }
}

fn texel(source: &[u8], out: &mut [u8]) {
    match *source {
        [gray] => out[0] = gray,
        [low, high] if out.len() == 2 => out.copy_from_slice(&[low, high]),
        [low, high] => {
            let packed = u16::from_le_bytes([low, high]);
            let channel = |shift: u16| {
                let five = (packed >> shift) & 0x1F;
                ((five << 3) | (five >> 2)) as u8
            };
            out.copy_from_slice(&[channel(10), channel(5), channel(0)]);
        }
        [blue, green, red] => out.copy_from_slice(&[red, green, blue]),
        [blue, green, red, alpha] => out.copy_from_slice(&[red, green, blue, alpha]),
        _ => {}
    }
}

pub(crate) fn probe(data: &[u8]) -> Result<Probe> {
    Header::read(&mut Reader::new(Format::Tga, data))?.probe()
}

pub(crate) fn decode(data: &[u8], limits: &Limits) -> Result<Image> {
    let mut reader = Reader::new(Format::Tga, data);
    let header = Header::read(&mut reader)?;
    let Probe {
        width,
        height,
        color,
    } = header.probe()?;
    let bytes = limits.admit(width, height, color)?;
    plausible(Format::Tga, bytes, data.len(), MOST_EXPANSION)?;
    reader.skip(header.id_length)?;
    let channels = usize::from(color.channel_count());
    let stored = usize::from(header.bits).div_ceil(8);

    let mut map = Vec::new();
    if header.map_length > 0 {
        let entry = usize::from(header.map_entry_bits).div_ceil(8);
        let entries = reader.take(header.map_length * entry)?;
        if header.kind == COLOR_MAPPED {
            map.resize(header.map_length * channels, 0);
            for (target, source) in map
                .chunks_exact_mut(channels)
                .zip(entries.chunks_exact(entry))
            {
                texel(source, target);
            }
        }
    }
    let write = |source: &[u8], target: &mut [u8]| -> Result<()> {
        if header.kind != COLOR_MAPPED {
            texel(source, target);
            return Ok(());
        }
        let index = match *source {
            [index] => usize::from(index),
            [low, high] => usize::from(u16::from_le_bytes([low, high])),
            _ => 0,
        };
        let entry = index
            .checked_sub(header.map_first)
            .and_then(|index| map.get(index * channels..(index + 1) * channels))
            .ok_or_else(|| damaged(Format::Tga, "colour index outside the map"))?;
        target.copy_from_slice(entry);
        Ok(())
    };

    let mut out = vec![0u8; bytes];
    if header.compressed {
        let mut targets = out.chunks_exact_mut(channels);
        while targets.len() > 0 {
            let packet = reader.u8()?;
            let count = usize::from(packet & !RUN_PACKET) + 1;
            if packet & RUN_PACKET != 0 {
                let source = reader.take(stored)?;
                for target in targets.by_ref().take(count) {
                    write(source, target)?;
                }
            } else {
                for target in targets.by_ref().take(count) {
                    write(reader.take(stored)?, target)?;
                }
            }
        }
    } else {
        let sources = reader.take(width as usize * height as usize * stored)?;
        for (target, source) in out
            .chunks_exact_mut(channels)
            .zip(sources.chunks_exact(stored))
        {
            write(source, target)?;
        }
    }

    let pitch = width as usize * channels;
    if header.descriptor & TOP_TO_BOTTOM == 0 {
        let (mut top, mut bottom) = (0, height as usize - 1);
        while top < bottom {
            let (upper, lower) = out.split_at_mut(bottom * pitch);
            upper[top * pitch..(top + 1) * pitch].swap_with_slice(&mut lower[..pitch]);
            top += 1;
            bottom -= 1;
        }
    }
    if header.descriptor & RIGHT_TO_LEFT != 0 {
        for row in out.chunks_exact_mut(pitch) {
            let (mut left, mut right) = (0, width as usize - 1);
            while left < right {
                for channel in 0..channels {
                    row.swap(left * channels + channel, right * channels + channel);
                }
                left += 1;
                right -= 1;
            }
        }
    }
    Ok(Image::from_pixels(width, height, color, Pixels::U8(out)))
}

pub(crate) fn encode(image: &Image, out: &mut Vec<u8>) -> Result<()> {
    let (width, height) = image.dimensions();
    let (Ok(narrow_width), Ok(narrow_height)) = (u16::try_from(width), u16::try_from(height))
    else {
        return Err(Error::Encoding(format!(
            "TGA: {width}x{height} exceeds 65535 pixels a side"
        )));
    };
    let color = image.color();
    let channels = usize::from(color.channel_count());
    let kind = if color.has_color() { TRUE_COLOR } else { GRAY };
    let alpha_bits = if color.has_alpha() { 8 } else { 0 };
    out.extend_from_slice(&[0, 0, kind | RUN_LENGTH, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&narrow_width.to_le_bytes());
    out.extend_from_slice(&narrow_height.to_le_bytes());
    out.extend_from_slice(&[(channels * 8) as u8, TOP_TO_BOTTOM | alpha_bits]);

    let mut stored = Vec::with_capacity(width as usize * channels);
    for row in image.as_bytes().chunks_exact(width as usize * channels) {
        stored.clear();
        for pixel in row.chunks_exact(channels) {
            match *pixel {
                [red, green, blue] => stored.extend_from_slice(&[blue, green, red]),
                [red, green, blue, alpha] => stored.extend_from_slice(&[blue, green, red, alpha]),
                _ => stored.extend_from_slice(pixel),
            }
        }
        let pixels: Vec<&[u8]> = stored.chunks_exact(channels).collect();
        let mut at = 0;
        while at < pixels.len() {
            let same = pixels[at..]
                .iter()
                .take(LONGEST_PACKET)
                .take_while(|&&pixel| pixel == pixels[at])
                .count();
            if same > 1 {
                out.push(RUN_PACKET | (same - 1) as u8);
                out.extend_from_slice(pixels[at]);
                at += same;
                continue;
            }
            let mut end = at + 1;
            while end < pixels.len()
                && end - at < LONGEST_PACKET
                && (end + 1 >= pixels.len() || pixels[end] != pixels[end + 1])
            {
                end += 1;
            }
            out.push((end - at - 1) as u8);
            for pixel in &pixels[at..end] {
                out.extend_from_slice(pixel);
            }
            at = end;
        }
    }
    Ok(())
}
