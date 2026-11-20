use crate::codecs::{Limits, Probe, Reader, damaged, plausible, unsupported};
use crate::color_type::ColorType;
use crate::error::Result;
use crate::format::Format;
use crate::pixels::Pixels;
use crate::raster::Image;

const MAGIC: &[u8; 4] = b"DDS ";
const HEADER_SIZE: u32 = 124;
const PIXEL_FORMAT_SIZE: u32 = 32;
const ALPHA_PIXELS: u32 = 0x1;
const FOUR_CC: u32 = 0x4;
const RGB: u32 = 0x40;
const LUMINANCE: u32 = 0x2_0000;
const EXTENDED: &[u8; 4] = b"DX10";
const BLOCK: usize = 4;
const MOST_EXPANSION: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Blocks {
    Bc1,
    Bc2,
    Bc3,
}

#[derive(Clone, Copy, Debug)]
enum Encoding {
    Blocks(Blocks),
    Masked { bytes: usize, masks: [u32; 4] },
    Gray { alpha: bool },
}

#[derive(Clone, Copy, Debug)]
struct Header {
    width: u32,
    height: u32,
    encoding: Encoding,
}

impl Header {
    fn read(reader: &mut Reader<'_>) -> Result<Header> {
        let invalid = || damaged(Format::Dds, "invalid header");
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(damaged(Format::Dds, "missing signature"));
        }
        if reader.u32_le()? != HEADER_SIZE {
            return Err(invalid());
        }
        reader.skip(4)?;
        let height = reader.u32_le()?;
        let width = reader.u32_le()?;
        reader.skip(4 * 14)?;
        if reader.u32_le()? != PIXEL_FORMAT_SIZE {
            return Err(invalid());
        }
        let flags = reader.u32_le()?;
        let four_cc: [u8; 4] = reader.array()?;
        let bits = reader.u32_le()?;
        let mut masks = [0u32; 4];
        for mask in &mut masks {
            *mask = reader.u32_le()?;
        }
        reader.skip(4 * 5)?;
        let other = || unsupported(Format::Dds, "pixel format");
        let encoding = if flags & FOUR_CC != 0 {
            match &four_cc {
                b"DXT1" => Encoding::Blocks(Blocks::Bc1),
                b"DXT2" | b"DXT3" => Encoding::Blocks(Blocks::Bc2),
                b"DXT4" | b"DXT5" => Encoding::Blocks(Blocks::Bc3),
                EXTENDED => {
                    let format = reader.u32_le()?;
                    reader.skip(4 * 4)?;
                    match format {
                        70..=72 => Encoding::Blocks(Blocks::Bc1),
                        73..=75 => Encoding::Blocks(Blocks::Bc2),
                        76..=78 => Encoding::Blocks(Blocks::Bc3),
                        27..=29 => Encoding::Masked {
                            bytes: 4,
                            masks: [0xFF, 0xFF00, 0xFF_0000, 0xFF00_0000],
                        },
                        87 | 90 | 91 => Encoding::Masked {
                            bytes: 4,
                            masks: [0xFF_0000, 0xFF00, 0xFF, 0xFF00_0000],
                        },
                        88 | 92 | 93 => Encoding::Masked {
                            bytes: 4,
                            masks: [0xFF_0000, 0xFF00, 0xFF, 0],
                        },
                        _ => return Err(other()),
                    }
                }
                _ => return Err(other()),
            }
        } else if flags & RGB != 0 && matches!(bits, 16 | 24 | 32) {
            if flags & ALPHA_PIXELS == 0 {
                masks[3] = 0;
            }
            Encoding::Masked {
                bytes: bits as usize / 8,
                masks,
            }
        } else if flags & LUMINANCE != 0 && matches!(bits, 8 | 16) {
            Encoding::Gray { alpha: bits == 16 }
        } else {
            return Err(other());
        };
        Ok(Header {
            width,
            height,
            encoding,
        })
    }

    fn probe(&self) -> Probe {
        let color = match self.encoding {
            Encoding::Blocks(Blocks::Bc1) => ColorType::Rgb8,
            Encoding::Blocks(_) => ColorType::Rgba8,
            Encoding::Masked { masks, .. } if masks[3] != 0 => ColorType::Rgba8,
            Encoding::Masked { .. } => ColorType::Rgb8,
            Encoding::Gray { alpha: true } => ColorType::La8,
            Encoding::Gray { alpha: false } => ColorType::L8,
        };
        Probe {
            width: self.width,
            height: self.height,
            color,
        }
    }
}

pub(crate) fn probe(data: &[u8]) -> Result<Probe> {
    Ok(Header::read(&mut Reader::new(Format::Dds, data))?.probe())
}

#[inline]
fn expand_565(packed: u16) -> [u8; 3] {
    let red = (packed >> 11) & 0x1F;
    let green = (packed >> 5) & 0x3F;
    let blue = packed & 0x1F;
    [
        (red << 3 | red >> 2) as u8,
        (green << 2 | green >> 4) as u8,
        (blue << 3 | blue >> 2) as u8,
    ]
}

fn color_block(block: &[u8; 8], punch_through: bool) -> [[u8; 3]; 16] {
    let first = u16::from_le_bytes([block[0], block[1]]);
    let second = u16::from_le_bytes([block[2], block[3]]);
    let (near, far) = (expand_565(first), expand_565(second));
    let mix = |near_weight: u16, far_weight: u16| -> [u8; 3] {
        let total = near_weight + far_weight;
        [0, 1, 2].map(|channel| {
            ((u16::from(near[channel]) * near_weight + u16::from(far[channel]) * far_weight)
                / total) as u8
        })
    };
    let palette = if first > second || !punch_through {
        [near, far, mix(2, 1), mix(1, 2)]
    } else {
        [near, far, mix(1, 1), [0; 3]]
    };
    let selectors = u32::from_le_bytes([block[4], block[5], block[6], block[7]]);
    std::array::from_fn(|texel| palette[(selectors >> (texel * 2) & 3) as usize])
}

fn alpha_block(block: &[u8; 8], blocks: Blocks) -> [u8; 16] {
    let bits = u64::from_le_bytes(*block);
    if blocks == Blocks::Bc2 {
        return std::array::from_fn(|texel| ((bits >> (texel * 4) & 0xF) as u8) * 17);
    }
    let (first, second) = (u16::from(block[0]), u16::from(block[1]));
    let mut ramp = [block[0], block[1], 0, 0, 0, 0, 0, 255];
    let steps: u16 = if first > second { 7 } else { 5 };
    for step in 1..steps {
        ramp[usize::from(step) + 1] = (((steps - step) * first + step * second) / steps) as u8;
    }
    let selectors = bits >> 16;
    std::array::from_fn(|texel| ramp[(selectors >> (texel * 3) & 7) as usize])
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
    fn read(self, word: u32) -> u8 {
        if self.most == 0 {
            return 0;
        }
        let value = u64::from((word & self.mask) >> self.shift);
        (value * 255 / u64::from(self.most)) as u8
    }
}

pub(crate) fn decode(data: &[u8], limits: &Limits) -> Result<Image> {
    let mut reader = Reader::new(Format::Dds, data);
    let header = Header::read(&mut reader)?;
    let Probe {
        width,
        height,
        color,
    } = header.probe();
    let bytes = limits.admit(width, height, color)?;
    plausible(Format::Dds, bytes, data.len(), MOST_EXPANSION)?;
    let channels = usize::from(color.channel_count());
    let (columns, rows) = (width as usize, height as usize);
    let mut out = vec![0u8; bytes];
    match header.encoding {
        Encoding::Blocks(blocks) => {
            let across = columns.div_ceil(BLOCK);
            let down = rows.div_ceil(BLOCK);
            let stored = if blocks == Blocks::Bc1 { 8 } else { 16 };
            let source = reader.take(across * down * stored)?;
            for (index, block) in source.chunks_exact(stored).enumerate() {
                let (alpha, colors) = block.split_at(stored - 8);
                let colors: &[u8; 8] = colors.try_into().unwrap_or(&[0; 8]);
                let texels = color_block(colors, blocks == Blocks::Bc1);
                let alphas = match alpha.try_into() {
                    Ok(alpha) => alpha_block(alpha, blocks),
                    Err(_) => [255; 16],
                };
                let (left, top) = (index % across * BLOCK, index / across * BLOCK);
                for y in 0..BLOCK.min(rows - top) {
                    for x in 0..BLOCK.min(columns - left) {
                        let at = ((top + y) * columns + left + x) * channels;
                        let texel = y * BLOCK + x;
                        out[at..at + 3].copy_from_slice(&texels[texel]);
                        if channels == 4 {
                            out[at + 3] = alphas[texel];
                        }
                    }
                }
            }
        }
        Encoding::Masked { bytes, masks } => {
            let fields = masks.map(Field::new);
            let source = reader.take(columns * rows * bytes)?;
            for (target, texel) in out
                .chunks_exact_mut(channels)
                .zip(source.chunks_exact(bytes))
            {
                let mut word = [0u8; 4];
                word[..bytes].copy_from_slice(texel);
                let word = u32::from_le_bytes(word);
                for (channel, value) in target.iter_mut().enumerate() {
                    *value = fields[channel].read(word);
                }
            }
        }
        Encoding::Gray { .. } => out.copy_from_slice(reader.take(bytes)?),
    }
    Ok(Image::from_pixels(width, height, color, Pixels::U8(out)))
}
