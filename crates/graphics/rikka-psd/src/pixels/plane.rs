use std::ops::Range;

use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::pixels::quantize::{quantize_u8, quantize_u16};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Depth {
    One,
    Eight,
    Sixteen,
    ThirtyTwo,
}

impl Depth {
    pub fn from_bits(bits: u16) -> Option<Self> {
        match bits {
            1 => Some(Depth::One),
            8 => Some(Depth::Eight),
            16 => Some(Depth::Sixteen),
            32 => Some(Depth::ThirtyTwo),
            _ => None,
        }
    }

    pub fn bits(self) -> u16 {
        match self {
            Depth::One => 1,
            Depth::Eight => 8,
            Depth::Sixteen => 16,
            Depth::ThirtyTwo => 32,
        }
    }

    pub fn row_bytes(self, width: u32) -> usize {
        (width as usize * usize::from(self.bits())).div_ceil(8)
    }

    pub fn plane_bytes(self, width: u32, height: u32) -> u64 {
        self.row_bytes(width) as u64 * u64::from(height)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Samples {
    Bits(Vec<u8>),
    U8(Vec<u8>),
    U16(Vec<u16>),
    F32(Vec<f32>),
}

impl Samples {
    pub fn depth(&self) -> Depth {
        match self {
            Samples::Bits(_) => Depth::One,
            Samples::U8(_) => Depth::Eight,
            Samples::U16(_) => Depth::Sixteen,
            Samples::F32(_) => Depth::ThirtyTwo,
        }
    }

    fn len(&self) -> usize {
        match self {
            Samples::Bits(values) | Samples::U8(values) => values.len(),
            Samples::U16(values) => values.len(),
            Samples::F32(values) => values.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plane {
    width: u32,
    height: u32,
    samples: Samples,
}

impl Plane {
    pub fn new(width: u32, height: u32, samples: Samples) -> Result<Self> {
        let expected = match samples {
            Samples::Bits(_) => Depth::One.row_bytes(width) * height as usize,
            _ => width as usize * height as usize,
        };
        if samples.len() != expected {
            return Err(Error::Planes("the sample count does not match the size"));
        }
        Ok(Self {
            width,
            height,
            samples,
        })
    }

    pub fn filled(width: u32, height: u32, depth: Depth, value: f32) -> Self {
        let count = width as usize * height as usize;
        let samples = match depth {
            Depth::One => {
                let byte = if value < 0.5 { 0xff } else { 0x00 };
                Samples::Bits(vec![byte; Depth::One.row_bytes(width) * height as usize])
            }
            Depth::Eight => Samples::U8(vec![to_u8(value); count]),
            Depth::Sixteen => Samples::U16(vec![to_u16(value); count]),
            Depth::ThirtyTwo => Samples::F32(vec![value; count]),
        };
        Self {
            width,
            height,
            samples,
        }
    }

    pub fn from_normalized(width: u32, height: u32, depth: Depth, values: &[f32]) -> Result<Self> {
        if values.len() != width as usize * height as usize {
            return Err(Error::Planes("the sample count does not match the size"));
        }
        let samples = match depth {
            Depth::One => {
                let row = Depth::One.row_bytes(width);
                let mut bits = vec![0u8; row * height as usize];
                for y in 0..height as usize {
                    for x in 0..width as usize {
                        if values[y * width as usize + x] < 0.5 {
                            bits[y * row + x / 8] |= 0x80 >> (x % 8);
                        }
                    }
                }
                Samples::Bits(bits)
            }
            Depth::Eight => {
                let mut samples = vec![0u8; values.len()];
                quantize_u8(values, &mut samples);
                Samples::U8(samples)
            }
            Depth::Sixteen => {
                let mut samples = vec![0u16; values.len()];
                quantize_u16(values, &mut samples);
                Samples::U16(samples)
            }
            Depth::ThirtyTwo => Samples::F32(values.to_vec()),
        };
        Ok(Self {
            width,
            height,
            samples,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn depth(&self) -> Depth {
        self.samples.depth()
    }

    pub fn samples(&self) -> &Samples {
        &self.samples
    }

    pub fn into_samples(self) -> Samples {
        self.samples
    }

    pub fn normalized(&self, x: u32, y: u32) -> f32 {
        let index = y as usize * self.width as usize + x as usize;
        match &self.samples {
            Samples::Bits(bits) => {
                let row = Depth::One.row_bytes(self.width);
                let byte = bits[y as usize * row + x as usize / 8];
                if byte & (0x80 >> (x % 8)) != 0 {
                    0.0
                } else {
                    1.0
                }
            }
            Samples::U8(values) => f32::from(values[index]) / 255.0,
            Samples::U16(values) => f32::from(values[index]) / 65535.0,
            Samples::F32(values) => values[index],
        }
    }

    pub fn to_normalized(&self) -> Vec<f32> {
        self.normalized_rows(0..self.height)
    }

    pub fn normalized_rows(&self, rows: Range<u32>) -> Vec<f32> {
        let end = rows.end.min(self.height);
        let rows = rows.start.min(end)..end;
        let width = self.width as usize;
        let span = rows.start as usize * width..rows.end as usize * width;
        match &self.samples {
            Samples::U8(values) => values[span].iter().map(|&v| f32::from(v) / 255.0).collect(),
            Samples::U16(values) => values[span]
                .iter()
                .map(|&v| f32::from(v) / 65535.0)
                .collect(),
            Samples::F32(values) => values[span].to_vec(),
            Samples::Bits(_) => rows
                .flat_map(|y| (0..self.width).map(move |x| self.normalized(x, y)))
                .collect(),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        match &self.samples {
            Samples::Bits(values) | Samples::U8(values) => values.clone(),
            Samples::U16(values) => values.iter().flat_map(|v| v.to_be_bytes()).collect(),
            Samples::F32(values) => values.iter().flat_map(|v| v.to_be_bytes()).collect(),
        }
    }

    pub fn from_bytes(width: u32, height: u32, depth: Depth, bytes: &[u8]) -> Result<Self> {
        let expected = depth.plane_bytes(width, height);
        if bytes.len() as u64 != expected {
            return Err(Error::Planes("the byte count does not match the size"));
        }
        let samples = match depth {
            Depth::One => Samples::Bits(bytes.to_vec()),
            Depth::Eight => Samples::U8(bytes.to_vec()),
            Depth::Sixteen => Samples::U16(
                bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|pair| u16::from_be_bytes(*pair))
                    .collect(),
            ),
            Depth::ThirtyTwo => Samples::F32(
                bytes
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|quad| f32::from_be_bytes(*quad))
                    .collect(),
            ),
        };
        Ok(Self {
            width,
            height,
            samples,
        })
    }

    pub fn cropped(&self, area: Rect) -> Result<Self> {
        if !Rect::from_size(0, 0, self.width, self.height).encloses(&area) {
            return Err(Error::Planes("the crop area lies outside the plane"));
        }
        Ok(self.reframed(area, 0.0))
    }

    pub fn reframed(&self, area: Rect, fill: f32) -> Self {
        let (width, height) = (area.width(), area.height());
        let mut out = Plane::filled(width, height, self.depth(), fill);
        let overlap = area.intersection(&Rect::from_size(0, 0, self.width, self.height));
        if overlap.is_empty() {
            return out;
        }
        let columns = overlap.width() as usize;
        let from = |x: i32, y: i32| (x as usize, y as usize);
        let to = |x: i32, y: i32| ((x - area.left) as usize, (y - area.top) as usize);
        fn copy<T: Copy>(
            source: &[T],
            target: &mut [T],
            (stride, to_stride): (usize, usize),
            (from, to): ((usize, usize), (usize, usize)),
            columns: usize,
        ) {
            let start = from.1 * stride + from.0;
            let at = to.1 * to_stride + to.0;
            target[at..at + columns].copy_from_slice(&source[start..start + columns]);
        }
        let strides = (self.width as usize, width as usize);
        for y in overlap.top..overlap.bottom {
            let ends = (from(overlap.left, y), to(overlap.left, y));
            match (&self.samples, &mut out.samples) {
                (Samples::U8(source), Samples::U8(target)) => {
                    copy(source, target, strides, ends, columns);
                }
                (Samples::U16(source), Samples::U16(target)) => {
                    copy(source, target, strides, ends, columns);
                }
                (Samples::F32(source), Samples::F32(target)) => {
                    copy(source, target, strides, ends, columns);
                }
                (Samples::Bits(source), Samples::Bits(target)) => {
                    let from_row = Depth::One.row_bytes(self.width);
                    let to_row = Depth::One.row_bytes(width);
                    let ((sx, sy), (tx, ty)) = ends;
                    for offset in 0..columns {
                        let (x, column) = (sx + offset, tx + offset);
                        let bit = 0x80 >> (column % 8);
                        let slot = &mut target[ty * to_row + column / 8];
                        if source[sy * from_row + x / 8] & (0x80 >> (x % 8)) != 0 {
                            *slot |= bit;
                        } else {
                            *slot &= !bit;
                        }
                    }
                }
                _ => unreachable!("reframed plane keeps depth"),
            }
        }
        out
    }

    pub fn is_uniform(&self, value: f32) -> bool {
        match &self.samples {
            Samples::U8(values) => values.iter().all(|&v| v == to_u8(value)),
            Samples::U16(values) => values.iter().all(|&v| v == to_u16(value)),
            Samples::F32(values) => values.iter().all(|&v| v == value),
            Samples::Bits(_) => self.to_normalized().iter().all(|&v| v == value),
        }
    }
}

pub(crate) fn to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

pub(crate) fn to_u16(value: f32) -> u16 {
    (value.clamp(0.0, 1.0) * 65535.0).round() as u16
}
