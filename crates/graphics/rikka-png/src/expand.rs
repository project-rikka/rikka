use crate::chunk::{PLTE, TRNS};
use crate::error::{Error, Result};
use crate::header::{Color, Header};
use crate::layout::Layout;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Transparency {
    None,
    Palette,
    Gray(u16),
    Rgb([u16; 3]),
}

impl Transparency {
    pub(crate) const fn adds_alpha(self) -> bool {
        !matches!(self, Transparency::None)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Palette {
    pub(crate) colors: [[u8; 4]; 256],
    pub(crate) len: usize,
}

impl Palette {
    pub(crate) const fn new() -> Self {
        Self {
            colors: [[0, 0, 0, 255]; 256],
            len: 0,
        }
    }

    pub(crate) fn read(&mut self, body: &[u8]) -> Result<()> {
        let (colors, rest) = body.as_chunks::<3>();
        if !rest.is_empty() || colors.is_empty() || colors.len() > 256 {
            return Err(Error::Chunk(PLTE));
        }
        self.len = colors.len();
        for (entry, rgb) in self.colors.iter_mut().zip(colors) {
            *entry = [rgb[0], rgb[1], rgb[2], 255];
        }
        Ok(())
    }
}

pub(crate) fn read_transparency(
    header: &Header,
    body: &[u8],
    palette: &mut Palette,
) -> Result<Transparency> {
    let word = |index: usize| u16::from_be_bytes([body[index * 2], body[index * 2 + 1]]);
    match header.color {
        Color::Gray if body.len() == 2 => Ok(Transparency::Gray(word(0))),
        Color::Rgb if body.len() == 6 => Ok(Transparency::Rgb([word(0), word(1), word(2)])),
        Color::Indexed if palette.len > 0 && body.len() <= palette.len => {
            for (entry, &alpha) in palette.colors.iter_mut().zip(body) {
                entry[3] = alpha;
            }
            Ok(Transparency::Palette)
        }
        _ => Err(Error::Chunk(TRNS)),
    }
}

pub(crate) fn is_verbatim(header: &Header, transparency: Transparency) -> bool {
    header.depth == 8
        && !header.interlaced
        && header.color != Color::Indexed
        && !transparency.adds_alpha()
}

pub(crate) fn native_row(
    header: &Header,
    palette: &Palette,
    transparency: Transparency,
    line: &[u8],
    out: &mut [u8],
) -> Result<()> {
    let depth = header.depth;
    match (header.color, depth, transparency) {
        (Color::Gray | Color::GrayAlpha | Color::Rgb | Color::Rgba, 8, Transparency::None) => {
            out.copy_from_slice(&line[..out.len()]);
        }
        (_, 16, Transparency::None) => {
            for (target, source) in out
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .zip(line.as_chunks().0)
            {
                *target = u16::from_be_bytes(*source).to_ne_bytes();
            }
        }
        (Color::Indexed, 8, Transparency::None) => {
            for (target, &index) in out.as_chunks_mut::<3>().0.iter_mut().zip(line) {
                let color = palette_color(palette, usize::from(index))?;
                target.copy_from_slice(&color[..3]);
            }
        }
        (Color::Indexed, 8, _) => {
            for (target, &index) in out.as_chunks_mut::<4>().0.iter_mut().zip(line) {
                *target = palette_color(palette, usize::from(index))?;
            }
        }
        (Color::Indexed, _, _) => {
            let channels = if transparency.adds_alpha() { 4 } else { 3 };
            for (column, target) in out.chunks_exact_mut(channels).enumerate() {
                let color = palette_color(palette, usize::from(sample(line, column, depth)))?;
                target.copy_from_slice(&color[..channels]);
            }
        }
        (Color::Gray, 16, Transparency::Gray(key)) => {
            for (target, source) in out
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(line.as_chunks().0)
            {
                let value = u16::from_be_bytes(*source);
                let alpha = if value == key { 0 } else { u16::MAX };
                target[..2].copy_from_slice(&value.to_ne_bytes());
                target[2..].copy_from_slice(&alpha.to_ne_bytes());
            }
        }
        (Color::Gray, _, Transparency::Gray(key)) => {
            for (column, target) in out.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                let value = sample(line, column, depth);
                *target = [widen(value, depth), if value == key { 0 } else { 255 }];
            }
        }
        (Color::Gray, _, _) => {
            for (column, target) in out.iter_mut().enumerate() {
                *target = widen(sample(line, column, depth), depth);
            }
        }
        (Color::Rgb, 16, Transparency::Rgb(key)) => {
            for (target, source) in out
                .as_chunks_mut::<8>()
                .0
                .iter_mut()
                .zip(line.as_chunks::<6>().0)
            {
                let rgb = [
                    u16::from_be_bytes([source[0], source[1]]),
                    u16::from_be_bytes([source[2], source[3]]),
                    u16::from_be_bytes([source[4], source[5]]),
                ];
                let alpha = if rgb == key { 0 } else { u16::MAX };
                for (channel, value) in rgb.into_iter().chain([alpha]).enumerate() {
                    target[channel * 2..channel * 2 + 2].copy_from_slice(&value.to_ne_bytes());
                }
            }
        }
        (Color::Rgb, _, Transparency::Rgb(key)) => {
            for (target, source) in out
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(line.as_chunks::<3>().0)
            {
                let keyed = source.map(u16::from) == key;
                *target = [source[0], source[1], source[2], if keyed { 0 } else { 255 }];
            }
        }
        _ => return Err(Error::Header),
    }
    Ok(())
}

#[inline]
fn palette_color(palette: &Palette, index: usize) -> Result<[u8; 4]> {
    if index >= palette.len {
        return Err(Error::Chunk(PLTE));
    }
    Ok(palette.colors[index])
}

#[inline]
fn sample(line: &[u8], index: usize, depth: u8) -> u16 {
    if depth == 8 {
        return u16::from(line[index]);
    }
    let depth = usize::from(depth);
    let bit = index * depth;
    let shift = 8 - depth - bit % 8;
    u16::from((line[bit / 8] >> shift) & ((1u8 << depth) - 1))
}

#[inline]
fn widen(value: u16, depth: u8) -> u8 {
    match depth {
        8 => value as u8,
        _ => (u32::from(value) * 255 / ((1u32 << depth) - 1)) as u8,
    }
}

#[inline]
fn narrow(value: [u8; 2]) -> u8 {
    ((u32::from(u16::from_ne_bytes(value)) + 128) / 257) as u8
}

pub(crate) fn rgba8_row(layout: Layout, native: &[u8], out: &mut [u8]) {
    let out = out.as_chunks_mut::<4>().0;
    match layout {
        Layout::Rgba8 => out.as_flattened_mut().copy_from_slice(native),
        Layout::Rgb8 => {
            for (target, rgb) in out.iter_mut().zip(native.as_chunks::<3>().0) {
                *target = [rgb[0], rgb[1], rgb[2], 255];
            }
        }
        Layout::La8 => {
            for (target, pair) in out.iter_mut().zip(native.as_chunks::<2>().0) {
                *target = [pair[0], pair[0], pair[0], pair[1]];
            }
        }
        Layout::L8 => {
            for (target, &gray) in out.iter_mut().zip(native) {
                *target = [gray, gray, gray, 255];
            }
        }
        Layout::Rgba16 => {
            for (target, source) in out.iter_mut().zip(native.as_chunks::<8>().0) {
                let (channels, _) = source.as_chunks::<2>();
                *target = [
                    narrow(channels[0]),
                    narrow(channels[1]),
                    narrow(channels[2]),
                    narrow(channels[3]),
                ];
            }
        }
        Layout::Rgb16 => {
            for (target, source) in out.iter_mut().zip(native.as_chunks::<6>().0) {
                let (channels, _) = source.as_chunks::<2>();
                *target = [
                    narrow(channels[0]),
                    narrow(channels[1]),
                    narrow(channels[2]),
                    255,
                ];
            }
        }
        Layout::La16 => {
            for (target, source) in out.iter_mut().zip(native.as_chunks::<4>().0) {
                let gray = narrow([source[0], source[1]]);
                *target = [gray, gray, gray, narrow([source[2], source[3]])];
            }
        }
        Layout::L16 => {
            for (target, source) in out.iter_mut().zip(native.as_chunks::<2>().0) {
                let gray = narrow(*source);
                *target = [gray, gray, gray, 255];
            }
        }
    }
}
