use rikka_compositor_geometry::{Buffer, Rect};
use rikka_compositor_types::Fourcc;

pub const SHM_FORMATS: [Fourcc; 11] = [
    Fourcc::ABGR8888,
    Fourcc::XBGR8888,
    Fourcc::ARGB8888,
    Fourcc::XRGB8888,
    Fourcc::ABGR2101010,
    Fourcc::XBGR2101010,
    Fourcc::ARGB2101010,
    Fourcc::XRGB2101010,
    Fourcc::ABGR16161616F,
    Fourcc::XBGR16161616F,
    Fourcc::RGB565,
];

#[derive(Clone, Copy)]
enum Layout {
    Bytes { swap_red_blue: bool },
    Ten { red_high: bool },
    Half,
    Rgb565,
}

#[derive(Clone, Copy)]
struct Packing {
    layout: Layout,
    opaque: bool,
    texel_bytes: usize,
}

fn packing(format: Fourcc) -> Option<Packing> {
    let (layout, opaque, texel_bytes) = match format {
        Fourcc::ABGR8888 => (
            Layout::Bytes {
                swap_red_blue: false,
            },
            false,
            4,
        ),
        Fourcc::XBGR8888 => (
            Layout::Bytes {
                swap_red_blue: false,
            },
            true,
            4,
        ),
        Fourcc::ARGB8888 => (
            Layout::Bytes {
                swap_red_blue: true,
            },
            false,
            4,
        ),
        Fourcc::XRGB8888 => (
            Layout::Bytes {
                swap_red_blue: true,
            },
            true,
            4,
        ),
        Fourcc::ABGR2101010 => (Layout::Ten { red_high: false }, false, 4),
        Fourcc::XBGR2101010 => (Layout::Ten { red_high: false }, true, 4),
        Fourcc::ARGB2101010 => (Layout::Ten { red_high: true }, false, 4),
        Fourcc::XRGB2101010 => (Layout::Ten { red_high: true }, true, 4),
        Fourcc::ABGR16161616F => (Layout::Half, false, 8),
        Fourcc::XBGR16161616F => (Layout::Half, true, 8),
        Fourcc::RGB565 => (Layout::Rgb565, true, 2),
        _ => return None,
    };
    Some(Packing {
        layout,
        opaque,
        texel_bytes,
    })
}

pub fn area_to_rgba(
    pixels: &[u8],
    format: Fourcc,
    stride: usize,
    area: Rect<i32, Buffer>,
) -> Option<Vec<u8>> {
    let packing = packing(format)?;
    let left = usize::try_from(area.loc.x).ok()?;
    let top = usize::try_from(area.loc.y).ok()?;
    let width = usize::try_from(area.size.w).ok()?;
    let height = usize::try_from(area.size.h).ok()?;
    let row_bytes = width * packing.texel_bytes;
    let first = left * packing.texel_bytes;
    let need = match height {
        0 => 0,
        height => (top + height - 1) * stride + first + row_bytes,
    };
    if stride < first + row_bytes || pixels.len() < need {
        return None;
    }
    let mut out = vec![0u8; width * height * 4];
    for (row, to) in out.chunks_exact_mut((width * 4).max(1)).enumerate() {
        let start = (top + row) * stride + first;
        let from = &pixels[start..start + row_bytes];
        convert_row(packing.layout, from, to);
        if packing.opaque {
            to.iter_mut()
                .skip(3)
                .step_by(4)
                .for_each(|alpha| *alpha = u8::MAX);
        }
    }
    Some(out)
}

fn convert_row(layout: Layout, from: &[u8], to: &mut [u8]) {
    let texels = to.as_chunks_mut::<4>().0;
    match layout {
        Layout::Bytes { swap_red_blue } => {
            for (texel, source) in texels.iter_mut().zip(from.as_chunks::<4>().0) {
                *texel = match swap_red_blue {
                    true => [source[2], source[1], source[0], source[3]],
                    false => *source,
                };
            }
        }
        Layout::Ten { red_high } => {
            for (texel, word) in texels.iter_mut().zip(from.as_chunks::<4>().0) {
                let word = u32::from_le_bytes(*word);
                let channel = |shift: u32| ten_bit_to_u8((word >> shift) & 0x3ff);
                let (red, blue) = match red_high {
                    true => (channel(20), channel(0)),
                    false => (channel(0), channel(20)),
                };
                *texel = [red, channel(10), blue, ((word >> 30) as u8) * 85];
            }
        }
        Layout::Half => {
            for (texel, halves) in texels.iter_mut().zip(from.as_chunks::<8>().0) {
                let channel = |at: usize| {
                    unit_to_u8(half_to_f32(u16::from_le_bytes([
                        halves[at],
                        halves[at + 1],
                    ])))
                };
                *texel = [channel(0), channel(2), channel(4), channel(6)];
            }
        }
        Layout::Rgb565 => {
            for (texel, word) in texels.iter_mut().zip(from.as_chunks::<2>().0) {
                let word = u32::from(u16::from_le_bytes(*word));
                let scale = |value: u32, most: u32| ((value * 255 + most / 2) / most) as u8;
                *texel = [
                    scale(word >> 11, 31),
                    scale((word >> 5) & 0x3f, 63),
                    scale(word & 0x1f, 31),
                    u8::MAX,
                ];
            }
        }
    }
}

fn ten_bit_to_u8(value: u32) -> u8 {
    ((value * 255 + 511) / 1023) as u8
}

fn half_to_f32(half: u16) -> f32 {
    let sign = u32::from(half >> 15) << 31;
    let exponent = u32::from(half >> 10) & 0x1f;
    let fraction = u32::from(half) & 0x3ff;
    let magnitude = match (exponent, fraction) {
        (0, 0) => 0,
        (0, fraction) => {
            let shift = fraction.leading_zeros() - 21;
            ((113 - shift) << 23) | (((fraction << shift) & 0x3ff) << 13)
        }
        (0x1f, fraction) => 0x7f80_0000 | (fraction << 13),
        (exponent, fraction) => ((exponent + 112) << 23) | (fraction << 13),
    };
    f32::from_bits(sign | magnitude)
}

fn unit_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}
