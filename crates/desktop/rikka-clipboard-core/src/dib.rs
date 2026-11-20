const INFO_HEADER: usize = 40;
const V4_HEADER: usize = 108;
const V5_HEADER: usize = 124;
const BI_RGB: u32 = 0;
const BI_BITFIELDS: u32 = 3;
const LCS_SRGB: u32 = 0x7352_4742;
const LCS_GM_IMAGES: u32 = 4;
const RED: u32 = 0x00FF_0000;
const GREEN: u32 = 0x0000_FF00;
const BLUE: u32 = 0x0000_00FF;
const ALPHA: u32 = 0xFF00_0000;
const MAX_SIDE: u32 = 1 << 15;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[must_use]
pub fn encode_v5(picture: &Rgba) -> Option<Vec<u8>> {
    let pixels = (picture.width as usize)
        .checked_mul(picture.height as usize)?
        .checked_mul(4)?;
    if pixels == 0 || picture.pixels.len() != pixels {
        return None;
    }
    let width = i32::try_from(picture.width).ok()?;
    let height = i32::try_from(picture.height).ok()?;
    let mut out = Vec::with_capacity(V5_HEADER + pixels);
    put_u32(&mut out, V5_HEADER as u32);
    put_i32(&mut out, width);
    put_i32(&mut out, height);
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    put_u32(&mut out, BI_BITFIELDS);
    put_u32(&mut out, u32::try_from(pixels).ok()?);
    put_i32(&mut out, 0);
    put_i32(&mut out, 0);
    put_u32(&mut out, 0);
    put_u32(&mut out, 0);
    for mask in [RED, GREEN, BLUE, ALPHA, LCS_SRGB] {
        put_u32(&mut out, mask);
    }
    out.resize(out.len() + 36 + 12, 0);
    put_u32(&mut out, LCS_GM_IMAGES);
    put_u32(&mut out, 0);
    put_u32(&mut out, 0);
    put_u32(&mut out, 0);
    let row = picture.width as usize * 4;
    for line in picture.pixels.chunks_exact(row).rev() {
        for &[red, green, blue, opacity] in line.as_chunks::<4>().0 {
            out.extend_from_slice(&[blue, green, red, opacity]);
        }
    }
    Some(out)
}

#[must_use]
pub fn decode(bytes: &[u8]) -> Option<Rgba> {
    let header = u32_at(bytes, 0)? as usize;
    if !matches!(header, INFO_HEADER | V4_HEADER | V5_HEADER) {
        return None;
    }
    let width = i32_at(bytes, 4)?;
    let height = i32_at(bytes, 8)?;
    let bits = u16_at(bytes, 14)?;
    let compression = u32_at(bytes, 16)?;
    let colors = u32_at(bytes, 32)? as usize;
    let width = u32::try_from(width)
        .ok()
        .filter(|width| (1..=MAX_SIDE).contains(width))?;
    let rows = height.unsigned_abs();
    if !(1..=MAX_SIDE).contains(&rows) {
        return None;
    }
    let masks = match (compression, header) {
        (BI_RGB, _) => [RED, GREEN, BLUE, 0],
        (BI_BITFIELDS, INFO_HEADER) => [
            u32_at(bytes, header)?,
            u32_at(bytes, header + 4)?,
            u32_at(bytes, header + 8)?,
            0,
        ],
        (BI_BITFIELDS, _) => [
            u32_at(bytes, 40)?,
            u32_at(bytes, 44)?,
            u32_at(bytes, 48)?,
            u32_at(bytes, 52)?,
        ],
        _ => return None,
    };
    let fields = match (compression, header) {
        (BI_BITFIELDS, INFO_HEADER) => 12,
        _ => 0,
    };
    let start = header + fields + colors.checked_mul(4)?;
    let stride = match bits {
        32 => width as usize * 4,
        24 => (width as usize * 3).div_ceil(4) * 4,
        _ => return None,
    };
    let data = bytes.get(start..start.checked_add(stride.checked_mul(rows as usize)?)?)?;
    let mut pixels = Vec::with_capacity(width as usize * rows as usize * 4);
    let mut lines: Vec<&[u8]> = data.chunks_exact(stride).collect();
    if height > 0 {
        lines.reverse();
    }
    for line in lines {
        match bits {
            32 => {
                for word in line[..width as usize * 4].as_chunks::<4>().0 {
                    let value = u32::from_le_bytes(*word);
                    pixels.extend_from_slice(&[
                        channel(value, masks[0]),
                        channel(value, masks[1]),
                        channel(value, masks[2]),
                        channel(value, masks[3]),
                    ]);
                }
            }
            _ => {
                for &[blue, green, red] in line[..width as usize * 3].as_chunks::<3>().0 {
                    pixels.extend_from_slice(&[red, green, blue, u8::MAX]);
                }
            }
        }
    }
    let opacity_unused = bits == 32 && pixels.as_chunks::<4>().0.iter().all(|pixel| pixel[3] == 0);
    if masks[3] == 0 || opacity_unused {
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel[3] = u8::MAX;
        }
    }
    Some(Rgba {
        width,
        height: rows,
        pixels,
    })
}

fn channel(value: u32, mask: u32) -> u8 {
    if mask == 0 {
        return 0;
    }
    let shifted = (value & mask) >> mask.trailing_zeros();
    let top = mask >> mask.trailing_zeros();
    ((u64::from(shifted) * 255 + u64::from(top) / 2) / u64::from(top)) as u8
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn i32_at(bytes: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}
