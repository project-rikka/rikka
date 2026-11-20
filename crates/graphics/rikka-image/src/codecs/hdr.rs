use crate::codecs::{Limits, Probe, Reader, damaged, plausible, unsupported};
use crate::color_type::ColorType;
use crate::error::Result;
use crate::format::Format;
use crate::pixels::Pixels;
use crate::raster::Image;

const SIGNATURES: [&[u8]; 2] = [b"#?RADIANCE", b"#?RGBE"];
const FORMAT_KEY: &[u8] = b"FORMAT=";
const RGBE_FORMAT: &[u8] = b"32-bit_rle_rgbe";
const MAX_HEADER_LINE: usize = 4096;
const RLE_WIDTHS: std::ops::RangeInclusive<usize> = 8..=32767;
const MIN_RUN: usize = 4;
const LONGEST_RUN: usize = 127;
const LONGEST_LITERAL: usize = 128;
const SMALLEST: f32 = 1e-32;
const MOST_EXPANSION: usize = 1024;

struct Header {
    width: u32,
    height: u32,
    bottom_up: bool,
}

fn line<'d>(reader: &mut Reader<'d>) -> Result<&'d [u8]> {
    let rest = reader.rest();
    let end = rest
        .iter()
        .take(MAX_HEADER_LINE)
        .position(|&byte| byte == b'\n')
        .ok_or_else(|| reader.short())?;
    reader.skip(end + 1)?;
    Ok(rest[..end].strip_suffix(b"\r").unwrap_or(&rest[..end]))
}

fn header(reader: &mut Reader<'_>) -> Result<Header> {
    let malformed = || damaged(Format::Hdr, "malformed header");
    let first = line(reader)?;
    if !SIGNATURES
        .iter()
        .any(|signature| first.starts_with(signature))
    {
        return Err(damaged(Format::Hdr, "missing signature"));
    }
    loop {
        let entry = line(reader)?;
        if entry.is_empty() {
            break;
        }
        if let Some(format) = entry.strip_prefix(FORMAT_KEY)
            && format != RGBE_FORMAT
        {
            return Err(unsupported(Format::Hdr, "pixel format other than RGBE"));
        }
    }
    let resolution = std::str::from_utf8(line(reader)?).map_err(|_| malformed())?;
    let mut parts = resolution.split_ascii_whitespace();
    let mut part = || parts.next().ok_or_else(malformed);
    let vertical = part()?;
    let height: u32 = part()?.parse().map_err(|_| malformed())?;
    let horizontal = part()?;
    let width: u32 = part()?.parse().map_err(|_| malformed())?;
    let bottom_up = match (vertical, horizontal) {
        ("-Y", "+X") => false,
        ("+Y", "+X") => true,
        _ => return Err(unsupported(Format::Hdr, "scanline orientation")),
    };
    Ok(Header {
        width,
        height,
        bottom_up,
    })
}

pub(crate) fn probe(data: &[u8]) -> Result<Probe> {
    let header = header(&mut Reader::new(Format::Hdr, data))?;
    Ok(Probe {
        width: header.width,
        height: header.height,
        color: ColorType::Rgb32F,
    })
}

#[inline]
fn radiance(rgbe: [u8; 4]) -> [f32; 3] {
    if rgbe[3] == 0 {
        return [0.0; 3];
    }
    let scale = 2f32.powi(i32::from(rgbe[3]) - 136);
    [
        f32::from(rgbe[0]) * scale,
        f32::from(rgbe[1]) * scale,
        f32::from(rgbe[2]) * scale,
    ]
}

fn packed_scanline(reader: &mut Reader<'_>, scanline: &mut [[u8; 4]]) -> Result<()> {
    let malformed = || damaged(Format::Hdr, "malformed run");
    for channel in 0..4 {
        let mut column = 0;
        while column < scanline.len() {
            let code = usize::from(reader.u8()?);
            let (count, run) = match code > LONGEST_LITERAL {
                true => (code - LONGEST_LITERAL, Some(reader.u8()?)),
                false => (code, None),
            };
            let targets = scanline
                .get_mut(column..column + count)
                .filter(|targets| !targets.is_empty())
                .ok_or_else(malformed)?;
            match run {
                Some(value) => targets.iter_mut().for_each(|pixel| pixel[channel] = value),
                None => {
                    for (pixel, &value) in targets.iter_mut().zip(reader.take(count)?) {
                        pixel[channel] = value;
                    }
                }
            }
            column += count;
        }
    }
    Ok(())
}

fn flat_scanline(reader: &mut Reader<'_>, first: [u8; 4], scanline: &mut [[u8; 4]]) -> Result<()> {
    let mut column = 0;
    let mut shift = 0u32;
    let mut pending = Some(first);
    while column < scanline.len() {
        let pixel = match pending.take() {
            Some(pixel) => pixel,
            None => reader.array()?,
        };
        if pixel[..3] == [1, 1, 1] && column > 0 {
            let count = (usize::from(pixel[3]) << shift.min(24)).min(scanline.len() - column);
            let previous = scanline[column - 1];
            scanline[column..column + count].fill(previous);
            column += count;
            shift += 8;
        } else {
            scanline[column] = pixel;
            column += 1;
            shift = 0;
        }
    }
    Ok(())
}

pub(crate) fn decode(data: &[u8], limits: &Limits) -> Result<Image> {
    let mut reader = Reader::new(Format::Hdr, data);
    let Header {
        width,
        height,
        bottom_up,
    } = header(&mut reader)?;
    let bytes = limits.admit(width, height, ColorType::Rgb32F)?;
    plausible(Format::Hdr, bytes, data.len(), MOST_EXPANSION)?;
    let columns = width as usize;
    let mut scanline = vec![[0u8; 4]; columns];
    let mut out = vec![0f32; columns * height as usize * 3];
    for row in 0..height as usize {
        let first: [u8; 4] = reader.array()?;
        let packed = RLE_WIDTHS.contains(&columns)
            && first[0] == 2
            && first[1] == 2
            && usize::from(first[2]) << 8 | usize::from(first[3]) == columns;
        match packed {
            true => packed_scanline(&mut reader, &mut scanline)?,
            false => flat_scanline(&mut reader, first, &mut scanline)?,
        }
        let row = if bottom_up {
            height as usize - 1 - row
        } else {
            row
        };
        let target = &mut out[row * columns * 3..(row + 1) * columns * 3];
        for (target, &rgbe) in target.as_chunks_mut::<3>().0.iter_mut().zip(&scanline) {
            *target = radiance(rgbe);
        }
    }
    Ok(Image::from_pixels(
        width,
        height,
        ColorType::Rgb32F,
        Pixels::F32(out),
    ))
}

#[inline]
fn shared_exponent(rgb: [f32; 3]) -> [u8; 4] {
    let brightest = rgb[0].max(rgb[1]).max(rgb[2]);
    if brightest.is_nan() || brightest < SMALLEST {
        return [0; 4];
    }
    let exponent = (((brightest.to_bits() >> 23) & 0xFF) as i32 - 126).clamp(-127, 127);
    let scale = 2f32.powi(8 - exponent);
    let mantissa = |value: f32| (value.max(0.0) * scale).min(255.0) as u8;
    [
        mantissa(rgb[0]),
        mantissa(rgb[1]),
        mantissa(rgb[2]),
        (exponent + 128) as u8,
    ]
}

fn pack(values: &[u8], out: &mut Vec<u8>) {
    let mut at = 0;
    while at < values.len() {
        let mut run_start = at;
        let mut run = 0;
        while run_start < values.len() {
            run = values[run_start..]
                .iter()
                .take(LONGEST_RUN)
                .take_while(|&&value| value == values[run_start])
                .count();
            if run >= MIN_RUN {
                break;
            }
            run_start += run;
        }
        while at < run_start {
            let count = (run_start - at).min(LONGEST_LITERAL);
            out.push(count as u8);
            out.extend_from_slice(&values[at..at + count]);
            at += count;
        }
        if run_start < values.len() {
            out.push((LONGEST_LITERAL + run) as u8);
            out.push(values[run_start]);
            at += run;
        }
    }
}

pub(crate) fn encode(image: &Image, out: &mut Vec<u8>) {
    let (width, height) = image.dimensions();
    out.extend_from_slice(SIGNATURES[0]);
    out.extend_from_slice(b"\nFORMAT=32-bit_rle_rgbe\n\n");
    out.extend_from_slice(format!("-Y {height} +X {width}\n").as_bytes());
    let Pixels::F32(samples) = &image.pixels else {
        unreachable!("converted to floating-point RGB before encoding");
    };
    let columns = width as usize;
    let mut scanline = vec![[0u8; 4]; columns];
    let mut plane = vec![0u8; columns];
    for row in samples.chunks_exact(columns * 3) {
        for (target, rgb) in scanline.iter_mut().zip(row.as_chunks::<3>().0) {
            *target = shared_exponent(*rgb);
        }
        if !RLE_WIDTHS.contains(&columns) {
            out.extend_from_slice(scanline.as_flattened());
            continue;
        }
        out.extend_from_slice(&[2, 2, (columns >> 8) as u8, columns as u8]);
        for channel in 0..4 {
            for (target, pixel) in plane.iter_mut().zip(&scanline) {
                *target = pixel[channel];
            }
            pack(&plane, out);
        }
    }
}
