use rikka_compositor_geometry::{Buffer, Rect};
use rikka_compositor_types::Fourcc;

const BYTES_PER_PIXEL: usize = 4;

const DEEP_OPAQUE: u32 = 0b11 << 30;
const DEEP_CHANNEL: u32 = 0x3ff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rows {
    AsRead,
    Opaque,
    Swapped { opaque: bool },
    DeepOpaque,
    DeepSwapped { opaque: bool },
}

impl Rows {
    fn for_format(format: Fourcc) -> Option<Self> {
        match format {
            Fourcc::ABGR8888 => Some(Self::AsRead),
            Fourcc::XBGR8888 => Some(Self::Opaque),
            Fourcc::ARGB8888 => Some(Self::Swapped { opaque: false }),
            Fourcc::XRGB8888 => Some(Self::Swapped { opaque: true }),
            _ => None,
        }
    }

    fn for_deep_format(format: Fourcc) -> Option<Self> {
        match format {
            Fourcc::ABGR2101010 => Some(Self::AsRead),
            Fourcc::XBGR2101010 => Some(Self::DeepOpaque),
            Fourcc::ARGB2101010 => Some(Self::DeepSwapped { opaque: false }),
            Fourcc::XRGB2101010 => Some(Self::DeepSwapped { opaque: true }),
            _ => None,
        }
    }

    fn write(self, into: &mut [u8], read: &[u8]) {
        match self {
            Self::AsRead => into.copy_from_slice(read),
            Self::Opaque => {
                into.copy_from_slice(read);
                for pixel in into.as_chunks_mut::<4>().0 {
                    pixel[3] = u8::MAX;
                }
            }
            Self::DeepOpaque => deep(into, read, |word| word | DEEP_OPAQUE),
            Self::DeepSwapped { opaque } => deep(into, read, |word| {
                let red = word & DEEP_CHANNEL;
                let blue = (word >> 20) & DEEP_CHANNEL;
                let alpha = match opaque {
                    true => DEEP_OPAQUE,
                    false => word & DEEP_OPAQUE,
                };
                alpha | red << 20 | (word & (DEEP_CHANNEL << 10)) | blue
            }),
            Self::Swapped { opaque } => {
                for (pixel, read) in into
                    .as_chunks_mut::<4>()
                    .0
                    .iter_mut()
                    .zip(read.as_chunks::<4>().0)
                {
                    pixel[0] = read[2];
                    pixel[1] = read[1];
                    pixel[2] = read[0];
                    pixel[3] = if opaque { u8::MAX } else { read[3] };
                }
            }
        }
    }
}

fn deep(into: &mut [u8], read: &[u8], convert: impl Fn(u32) -> u32) {
    for (pixel, read) in into
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(read.as_chunks::<4>().0)
    {
        *pixel = convert(u32::from_le_bytes(*read)).to_le_bytes();
    }
}

pub fn write_rows(
    into: &mut [u8],
    format: Fourcc,
    stride: usize,
    size: (usize, usize),
    rgba: &[u8],
    areas: impl IntoIterator<Item = Rect<i32, Buffer>>,
) -> bool {
    Rows::for_format(format).is_some_and(|rows| write_with(rows, into, stride, size, rgba, areas))
}

pub fn write_deep_rows(
    into: &mut [u8],
    format: Fourcc,
    stride: usize,
    size: (usize, usize),
    abgr2101010: &[u8],
    areas: impl IntoIterator<Item = Rect<i32, Buffer>>,
) -> bool {
    Rows::for_deep_format(format)
        .is_some_and(|rows| write_with(rows, into, stride, size, abgr2101010, areas))
}

fn write_with(
    rows: Rows,
    into: &mut [u8],
    stride: usize,
    (width, height): (usize, usize),
    read: &[u8],
    areas: impl IntoIterator<Item = Rect<i32, Buffer>>,
) -> bool {
    let row = width * BYTES_PER_PIXEL;
    if stride < row || into.len() < stride * height || read.len() < row * height {
        return false;
    }
    let within = |edge: i32, most: usize| usize::try_from(edge).map_or(0, |edge| edge.min(most));
    for area in areas {
        let (left, top) = (within(area.loc.x, width), within(area.loc.y, height));
        let right = within(area.loc.x.saturating_add(area.size.w), width);
        let bottom = within(area.loc.y.saturating_add(area.size.h), height);
        let (start, end) = (left * BYTES_PER_PIXEL, right * BYTES_PER_PIXEL);
        if start >= end {
            continue;
        }
        for y in top..bottom {
            rows.write(
                &mut into[y * stride + start..y * stride + end],
                &read[y * row + start..y * row + end],
            );
        }
    }
    true
}
