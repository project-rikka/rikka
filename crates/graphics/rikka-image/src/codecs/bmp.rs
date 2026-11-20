use crate::codecs::{Limits, Probe, Reader, damaged, plausible, unsupported};
use crate::color_type::ColorType;
use crate::error::{Error, Result};
use crate::format::Format;
use crate::pixels::Pixels;
use crate::raster::Image;

const FILE_HEADER: usize = 14;
const CORE_HEADER: u32 = 12;
const INFO_HEADER: u32 = 40;
const V4_HEADER: u32 = 108;
const RGB: u32 = 0;
const RLE8: u32 = 1;
const RLE4: u32 = 2;
const BITFIELDS: u32 = 3;
const ALPHA_BITFIELDS: u32 = 6;
const SRGB: u32 = 0x7352_4742;
const MASKS_555: [u32; 4] = [0x7C00, 0x03E0, 0x001F, 0];
const MASKS_888: [u32; 4] = [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0];
const ALPHA_MASK: u32 = 0xFF00_0000;
const ESCAPE: u8 = 0;
const END_OF_LINE: u8 = 0;
const END_OF_BITMAP: u8 = 1;
const DELTA: u8 = 2;
const MOST_EXPANSION: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Container {
    File,
    Icon,
}

#[derive(Clone, Debug)]
struct Header {
    width: u32,
    height: u32,
    top_down: bool,
    bits: u16,
    compression: u32,
    masks: [u32; 4],
    palette: Vec<[u8; 3]>,
    pixels_at: usize,
    container: Container,
}

impl Header {
    fn read(reader: &mut Reader<'_>, container: Container) -> Result<Header> {
        let invalid = || damaged(Format::Bmp, "invalid header");
        let start = reader.position();
        let mut pixels_at = None;
        if container == Container::File {
            if reader.take(2)? != b"BM" {
                return Err(damaged(Format::Bmp, "missing signature"));
            }
            reader.skip(8)?;
            pixels_at = Some(start + reader.u32_le()? as usize);
        }
        let header_at = reader.position();
        let header_size = reader.u32_le()?;
        let (width, height, bits, compression, colors) = if header_size == CORE_HEADER {
            let width = i32::from(reader.u16_le()?);
            let height = i32::from(reader.u16_le()?);
            reader.skip(2)?;
            (width, height, reader.u16_le()?, RGB, 0)
        } else if header_size >= INFO_HEADER {
            let width = reader.i32_le()?;
            let height = reader.i32_le()?;
            reader.skip(2)?;
            let bits = reader.u16_le()?;
            let compression = reader.u32_le()?;
            reader.skip(12)?;
            let colors = reader.u32_le()?;
            reader.skip(4)?;
            (width, height, bits, compression, colors)
        } else {
            return Err(invalid());
        };
        let height = match container {
            Container::File => height,
            Container::Icon => height / 2,
        };
        if width <= 0 || height == 0 || height == i32::MIN {
            return Err(invalid());
        }
        let mut masks = match bits {
            16 => MASKS_555,
            _ => MASKS_888,
        };
        if matches!(compression, BITFIELDS | ALPHA_BITFIELDS) {
            let inline = if header_size >= 56 || compression == ALPHA_BITFIELDS {
                4
            } else {
                3
            };
            for mask in masks.iter_mut().take(inline) {
                *mask = reader.u32_le()?;
            }
        } else if header_size >= 56 && bits == 32 {
            reader.skip(12)?;
            masks[3] = reader.u32_le()?;
        }
        if container == Container::Icon && bits == 32 && compression == RGB {
            masks[3] = ALPHA_MASK;
        }
        let masks_after_header =
            header_size == INFO_HEADER && matches!(compression, BITFIELDS | ALPHA_BITFIELDS);
        let palette_at = header_at
            + header_size as usize
            + match (masks_after_header, compression) {
                (true, ALPHA_BITFIELDS) => 16,
                (true, _) => 12,
                (false, _) => 0,
            };
        reader.seek(palette_at)?;
        let mut palette = Vec::new();
        if bits <= 8 {
            let entry = if header_size == CORE_HEADER { 3 } else { 4 };
            let declared = match colors {
                0 => 1usize << bits,
                colors => (colors as usize).min(1 << bits),
            };
            let room = pixels_at.map_or(usize::MAX, |at| at.saturating_sub(palette_at) / entry);
            for _ in 0..declared.min(room) {
                let [blue, green, red] = reader.array::<3>()?;
                reader.skip(entry - 3)?;
                palette.push([red, green, blue]);
            }
        }
        let supported = matches!(
            (bits, compression),
            (1 | 4 | 8 | 24, RGB)
                | (16 | 32, RGB | BITFIELDS | ALPHA_BITFIELDS)
                | (8, RLE8)
                | (4, RLE4)
        );
        if !supported {
            return Err(unsupported(Format::Bmp, "bit depth or compression"));
        }
        Ok(Header {
            width: width as u32,
            height: height.unsigned_abs(),
            top_down: height < 0,
            bits,
            compression,
            masks,
            pixels_at: pixels_at.unwrap_or(reader.position()),
            palette,
            container,
        })
    }

    fn color(&self) -> ColorType {
        let masked = self.bits > 8 && self.masks[3] != 0;
        let keyed = self.container == Container::Icon;
        match masked || keyed {
            true => ColorType::Rgba8,
            false => ColorType::Rgb8,
        }
    }

    fn probe(&self) -> Probe {
        Probe {
            width: self.width,
            height: self.height,
            color: self.color(),
        }
    }

    fn pitch(&self, bits: usize) -> usize {
        (self.width as usize * bits).div_ceil(32) * 4
    }

    fn indexed(&self, index: usize) -> [u8; 3] {
        self.palette.get(index).copied().unwrap_or([0; 3])
    }
}

#[derive(Clone, Copy)]
struct Field {
    mask: u32,
    shift: u32,
    most: u32,
}

impl Field {
    fn new(mask: u32) -> Field {
        let shift = mask.trailing_zeros() % 32;
        Field {
            mask,
            shift,
            most: mask >> shift,
        }
    }

    #[inline]
    fn read(self, word: u32, absent: u8) -> u8 {
        if self.most == 0 {
            return absent;
        }
        let value = u64::from((word & self.mask) >> self.shift);
        (value * 255 / u64::from(self.most)) as u8
    }
}

pub(crate) fn probe(data: &[u8]) -> Result<Probe> {
    Ok(Header::read(&mut Reader::new(Format::Bmp, data), Container::File)?.probe())
}

pub(crate) fn probe_icon(data: &[u8]) -> Result<Probe> {
    Ok(Header::read(&mut Reader::new(Format::Bmp, data), Container::Icon)?.probe())
}

pub(crate) fn decode(data: &[u8], limits: &Limits) -> Result<Image> {
    decode_in(data, limits, Container::File)
}

pub(crate) fn decode_icon(data: &[u8], limits: &Limits) -> Result<Image> {
    decode_in(data, limits, Container::Icon)
}

fn decode_in(data: &[u8], limits: &Limits, container: Container) -> Result<Image> {
    let mut reader = Reader::new(Format::Bmp, data);
    let header = Header::read(&mut reader, container)?;
    let color = header.color();
    let (width, height) = (header.width, header.height);
    let bytes = limits.admit(width, height, color)?;
    plausible(Format::Bmp, bytes, data.len(), MOST_EXPANSION)?;
    let channels = usize::from(color.channel_count());
    let columns = width as usize;
    let mut out = vec![0u8; bytes];
    if channels == 4 {
        out.as_chunks_mut::<4>()
            .0
            .iter_mut()
            .for_each(|pixel| pixel[3] = 255);
    }
    reader.seek(header.pixels_at)?;
    let target_row = |row: usize| match header.top_down {
        true => row,
        false => height as usize - 1 - row,
    };
    if matches!(header.compression, RLE8 | RLE4) {
        run_length(&mut reader, &header, channels, &mut out)?;
    } else {
        let bits = usize::from(header.bits);
        let pitch = header.pitch(bits);
        let fields = header.masks.map(Field::new);
        for row in 0..height as usize {
            let line = reader.take(pitch)?;
            let at = target_row(row) * columns * channels;
            let targets = out[at..at + columns * channels].chunks_exact_mut(channels);
            match bits {
                1 | 4 | 8 => {
                    for (column, target) in targets.enumerate() {
                        let bit = column * bits;
                        let index =
                            line[bit / 8] >> (8 - bits - bit % 8) & ((1u16 << bits) - 1) as u8;
                        target[..3].copy_from_slice(&header.indexed(usize::from(index)));
                    }
                }
                24 => {
                    for (target, bgr) in targets.zip(line.as_chunks::<3>().0) {
                        target[..3].copy_from_slice(&[bgr[2], bgr[1], bgr[0]]);
                    }
                }
                _ => {
                    let size = bits / 8;
                    for (target, source) in targets.zip(line.chunks_exact(size)) {
                        let mut word = [0u8; 4];
                        word[..size].copy_from_slice(source);
                        let word = u32::from_le_bytes(word);
                        let pixel = [
                            fields[0].read(word, 0),
                            fields[1].read(word, 0),
                            fields[2].read(word, 0),
                            fields[3].read(word, 255),
                        ];
                        target.copy_from_slice(&pixel[..channels]);
                    }
                }
            }
        }
    }
    if container == Container::Icon && header.masks[3] == 0 {
        let pitch = header.pitch(1);
        for row in 0..height as usize {
            let Ok(line) = reader.take(pitch) else {
                break;
            };
            let at = target_row(row) * columns * 4;
            for (column, pixel) in out[at..at + columns * 4]
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .enumerate()
            {
                if line[column / 8] >> (7 - column % 8) & 1 == 1 {
                    pixel[3] = 0;
                }
            }
        }
    }
    Ok(Image::from_pixels(width, height, color, Pixels::U8(out)))
}

fn run_length(
    reader: &mut Reader<'_>,
    header: &Header,
    channels: usize,
    out: &mut [u8],
) -> Result<()> {
    let (columns, rows) = (header.width as usize, header.height as usize);
    let nibbles = header.compression == RLE4;
    let (mut column, mut row) = (0usize, 0usize);
    let mut put = |column: usize, row: usize, index: u8| {
        if column < columns && row < rows {
            let line = match header.top_down {
                true => row,
                false => rows - 1 - row,
            };
            let at = (line * columns + column) * channels;
            out[at..at + 3].copy_from_slice(&header.indexed(usize::from(index)));
        }
    };
    let index_of = |byte: u8, position: usize| match (nibbles, position % 2) {
        (false, _) => byte,
        (true, 0) => byte >> 4,
        (true, _) => byte & 0x0F,
    };
    while row < rows {
        let [count, value] = reader.array::<2>()?;
        if count != ESCAPE {
            for position in 0..usize::from(count) {
                put(column, row, index_of(value, position));
                column += 1;
            }
            continue;
        }
        match value {
            END_OF_LINE => {
                column = 0;
                row += 1;
            }
            END_OF_BITMAP => break,
            DELTA => {
                let [right, down] = reader.array::<2>()?;
                column += usize::from(right);
                row += usize::from(down);
            }
            literal => {
                let literal = usize::from(literal);
                let stored = match nibbles {
                    true => literal.div_ceil(2),
                    false => literal,
                };
                let bytes = reader.take(stored.next_multiple_of(2))?;
                for position in 0..literal {
                    let byte = match nibbles {
                        true => bytes[position / 2],
                        false => bytes[position],
                    };
                    put(column, row, index_of(byte, position));
                    column += 1;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn encode(image: &Image, out: &mut Vec<u8>) -> Result<()> {
    let (width, height) = image.dimensions();
    let color = image.color();
    let channels = usize::from(color.channel_count());
    let (bits, palette, header_size): (u16, usize, u32) = match channels {
        1 => (8, 256, INFO_HEADER),
        3 => (24, 0, INFO_HEADER),
        _ => (32, 0, V4_HEADER),
    };
    let pitch = (width as usize * usize::from(bits)).div_ceil(32) * 4;
    let pixels_at = FILE_HEADER + header_size as usize + palette * 4;
    let file_size = u64::from(height) * pitch as u64 + pixels_at as u64;
    let (Ok(file_size), Ok(signed_width), Ok(signed_height)) = (
        u32::try_from(file_size),
        i32::try_from(width),
        i32::try_from(height),
    ) else {
        return Err(Error::Encoding(format!(
            "BMP: {width}x{height} image exceeds the 4 GiB file size"
        )));
    };
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(pixels_at as u32).to_le_bytes());
    out.extend_from_slice(&header_size.to_le_bytes());
    out.extend_from_slice(&signed_width.to_le_bytes());
    out.extend_from_slice(&signed_height.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    let compression = if header_size == V4_HEADER {
        BITFIELDS
    } else {
        RGB
    };
    out.extend_from_slice(&compression.to_le_bytes());
    out.extend_from_slice(&(file_size - pixels_at as u32).to_le_bytes());
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&(palette as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    if header_size == V4_HEADER {
        for mask in [MASKS_888[0], MASKS_888[1], MASKS_888[2], ALPHA_MASK, SRGB] {
            out.extend_from_slice(&mask.to_le_bytes());
        }
        out.extend_from_slice(&[0; 48]);
    }
    for gray in 0..palette {
        out.extend_from_slice(&[gray as u8, gray as u8, gray as u8, 0]);
    }
    let row = width as usize * channels;
    let stored = width as usize * usize::from(bits) / 8;
    let padding = [0u8; 3];
    for line in image.as_bytes().chunks_exact(row).rev() {
        match channels {
            1 => out.extend_from_slice(line),
            2 => {
                for pair in line.as_chunks::<2>().0 {
                    out.extend_from_slice(&[pair[0], pair[0], pair[0], pair[1]]);
                }
            }
            3 => {
                for rgb in line.as_chunks::<3>().0 {
                    out.extend_from_slice(&[rgb[2], rgb[1], rgb[0]]);
                }
            }
            _ => {
                for rgba in line.as_chunks::<4>().0 {
                    out.extend_from_slice(&[rgba[2], rgba[1], rgba[0], rgba[3]]);
                }
            }
        }
        out.extend_from_slice(&padding[..pitch - stored]);
    }
    Ok(())
}
