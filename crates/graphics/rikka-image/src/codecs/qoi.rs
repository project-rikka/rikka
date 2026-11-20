use crate::codecs::{Limits, Probe, Reader, damaged, plausible};
use crate::color_type::ColorType;
use crate::error::Result;
use crate::format::Format;
use crate::pixels::Pixels;
use crate::raster::Image;

const MAGIC: &[u8; 4] = b"qoif";
const END: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];
const OP_INDEX: u8 = 0x00;
const OP_DIFF: u8 = 0x40;
const OP_LUMA: u8 = 0x80;
const OP_RUN: u8 = 0xC0;
const OP_RGB: u8 = 0xFE;
const OP_RGBA: u8 = 0xFF;
const TAG_MASK: u8 = 0xC0;
const LONGEST_RUN: u8 = 62;
const START: [u8; 4] = [0, 0, 0, 255];
const MOST_EXPANSION: usize = 256;

#[inline]
fn slot(pixel: [u8; 4]) -> usize {
    let [red, green, blue, alpha] = pixel.map(usize::from);
    (red * 3 + green * 5 + blue * 7 + alpha * 11) % 64
}

fn header(reader: &mut Reader<'_>) -> Result<Probe> {
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(damaged(Format::Qoi, "missing signature"));
    }
    let width = reader.u32_be()?;
    let height = reader.u32_be()?;
    let color = match reader.u8()? {
        3 => ColorType::Rgb8,
        4 => ColorType::Rgba8,
        _ => return Err(damaged(Format::Qoi, "invalid channel count")),
    };
    reader.u8()?;
    Ok(Probe {
        width,
        height,
        color,
    })
}

pub(crate) fn probe(data: &[u8]) -> Result<Probe> {
    header(&mut Reader::new(Format::Qoi, data))
}

pub(crate) fn decode(data: &[u8], limits: &Limits) -> Result<Image> {
    let mut reader = Reader::new(Format::Qoi, data);
    let Probe {
        width,
        height,
        color,
    } = header(&mut reader)?;
    let bytes = limits.admit(width, height, color)?;
    plausible(Format::Qoi, bytes, data.len(), MOST_EXPANSION)?;
    let channels = usize::from(color.channel_count());
    let mut out = vec![0u8; bytes];
    let mut seen = [[0u8; 4]; 64];
    let mut pixel = START;
    let mut run = 0u8;
    for target in out.chunks_exact_mut(channels) {
        if run > 0 {
            run -= 1;
        } else {
            let op = reader.u8()?;
            match op {
                OP_RGB => pixel[..3].copy_from_slice(&reader.array::<3>()?),
                OP_RGBA => pixel = reader.array()?,
                _ => match op & TAG_MASK {
                    OP_INDEX => pixel = seen[usize::from(op)],
                    OP_DIFF => {
                        for (channel, shift) in [4u8, 2, 0].into_iter().enumerate() {
                            let delta = ((op >> shift) & 3).wrapping_sub(2);
                            pixel[channel] = pixel[channel].wrapping_add(delta);
                        }
                    }
                    OP_LUMA => {
                        let detail = reader.u8()?;
                        let green = (op & 0x3F).wrapping_sub(32);
                        let red = green.wrapping_add((detail >> 4).wrapping_sub(8));
                        let blue = green.wrapping_add((detail & 0x0F).wrapping_sub(8));
                        pixel[0] = pixel[0].wrapping_add(red);
                        pixel[1] = pixel[1].wrapping_add(green);
                        pixel[2] = pixel[2].wrapping_add(blue);
                    }
                    _ => run = op & 0x3F,
                },
            }
            seen[slot(pixel)] = pixel;
        }
        target.copy_from_slice(&pixel[..channels]);
    }
    Ok(Image::from_pixels(width, height, color, Pixels::U8(out)))
}

pub(crate) fn encode(image: &Image, out: &mut Vec<u8>) {
    let channels = usize::from(image.color().channel_count());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&image.width().to_be_bytes());
    out.extend_from_slice(&image.height().to_be_bytes());
    out.extend_from_slice(&[channels as u8, 0]);
    let mut seen = [[0u8; 4]; 64];
    let mut previous = START;
    let mut run = 0u8;
    for source in image.as_bytes().chunks_exact(channels) {
        let mut pixel = [0, 0, 0, previous[3]];
        pixel[..channels].copy_from_slice(source);
        if pixel == previous {
            run += 1;
            if run == LONGEST_RUN {
                out.push(OP_RUN | (run - 1));
                run = 0;
            }
            continue;
        }
        if run > 0 {
            out.push(OP_RUN | (run - 1));
            run = 0;
        }
        let at = slot(pixel);
        if seen[at] == pixel {
            out.push(OP_INDEX | at as u8);
        } else if pixel[3] != previous[3] {
            out.push(OP_RGBA);
            out.extend_from_slice(&pixel);
        } else {
            let red = pixel[0].wrapping_sub(previous[0]) as i8;
            let green = pixel[1].wrapping_sub(previous[1]) as i8;
            let blue = pixel[2].wrapping_sub(previous[2]) as i8;
            let red_green = red.wrapping_sub(green);
            let blue_green = blue.wrapping_sub(green);
            let small = |delta: i8| (-2..=1).contains(&delta);
            if small(red) && small(green) && small(blue) {
                out.push(
                    OP_DIFF | ((red + 2) as u8) << 4 | ((green + 2) as u8) << 2 | (blue + 2) as u8,
                );
            } else if (-32..=31).contains(&green)
                && (-8..=7).contains(&red_green)
                && (-8..=7).contains(&blue_green)
            {
                out.push(OP_LUMA | (green + 32) as u8);
                out.push(((red_green + 8) as u8) << 4 | (blue_green + 8) as u8);
            } else {
                out.push(OP_RGB);
                out.extend_from_slice(&pixel[..3]);
            }
        }
        seen[at] = pixel;
        previous = pixel;
    }
    if run > 0 {
        out.push(OP_RUN | (run - 1));
    }
    out.extend_from_slice(&END);
}
