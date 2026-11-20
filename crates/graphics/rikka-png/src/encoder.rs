use rikka_deflate::{Crc32, Deflater, Level};

use crate::chunk::{self, ICCP, IDAT, IEND, IHDR, MAX_CHUNK, SIGNATURE};
use crate::error::{Error, Result};
use crate::header::{Color, Header};
use crate::layout::Layout;
use crate::text::{self, Text};
use crate::unfilter::{AVERAGE, NONE, PAETH, SUB, UP, predictor};

const PROFILE_NAME: &[u8] = b"ICC Profile\0\0";
const FILTERS: [u8; 5] = [NONE, SUB, UP, AVERAGE, PAETH];

#[derive(Clone, Copy, Debug)]
pub struct Pixels<'p> {
    pub width: u32,
    pub height: u32,
    pub layout: Layout,
    pub bytes: &'p [u8],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Metadata<'m> {
    pub icc_profile: Option<&'m [u8]>,
    pub text: &'m [Text],
}

#[derive(Debug)]
pub struct Encoder {
    deflater: Deflater,
    filtered: Vec<u8>,
    rows: [Vec<u8>; 2],
    candidates: [Vec<u8>; FILTERS.len()],
}

impl Encoder {
    #[must_use]
    pub fn new(level: Level) -> Encoder {
        Encoder {
            deflater: Deflater::new(level),
            filtered: Vec::new(),
            rows: Default::default(),
            candidates: Default::default(),
        }
    }

    pub fn encode(
        &mut self,
        pixels: Pixels<'_>,
        metadata: &Metadata<'_>,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let Pixels {
            width,
            height,
            layout,
            bytes,
        } = pixels;
        let invalid = Error::InvalidSize { width, height };
        if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
            return Err(invalid);
        }
        let row = (width as usize)
            .checked_mul(layout.bytes_per_pixel())
            .ok_or(Error::InvalidSize { width, height })?;
        let expected = row.checked_mul(height as usize).ok_or(invalid)?;
        if bytes.len() != expected {
            return Err(Error::PixelDataSize {
                len: bytes.len(),
                expected,
            });
        }
        let header = Header {
            width: width as usize,
            height: height as usize,
            depth: if layout.is_deep() { 16 } else { 8 },
            color: match layout.channels() {
                1 => Color::Gray,
                2 => Color::GrayAlpha,
                3 => Color::Rgb,
                _ => Color::Rgba,
            },
            interlaced: false,
        };

        out.extend_from_slice(&SIGNATURE);
        chunk::write(out, IHDR, &header.encode())?;
        if let Some(profile) = metadata.icc_profile.filter(|profile| !profile.is_empty()) {
            let mut body = PROFILE_NAME.to_vec();
            self.deflater.zlib(profile, &mut body);
            chunk::write(out, ICCP, &body)?;
        }
        for (kind, body) in metadata.text.iter().filter_map(text::encode) {
            chunk::write(out, kind, &body)?;
        }

        self.filter(bytes, row, header.filter_stride(), layout.is_deep());
        let start = out.len();
        out.extend_from_slice(&[0; 4]);
        out.extend_from_slice(&IDAT.0);
        self.deflater.zlib(&self.filtered, out);
        let length = out.len() - start - 8;
        if length > MAX_CHUNK {
            return Err(Error::TooLarge);
        }
        out[start..start + 4].copy_from_slice(&(length as u32).to_be_bytes());
        let mut crc = Crc32::new();
        crc.update(&out[start + 4..]);
        out.extend_from_slice(&crc.finish().to_be_bytes());
        chunk::write(out, IEND, &[])
    }

    fn filter(&mut self, bytes: &[u8], row: usize, stride: usize, deep: bool) {
        self.filtered.clear();
        self.filtered.reserve(bytes.len() + bytes.len() / row);
        let [current, previous] = &mut self.rows;
        previous.clear();
        previous.resize(row, 0);
        current.clear();
        current.resize(row, 0);
        for candidate in &mut self.candidates {
            candidate.clear();
            candidate.resize(row, 0);
        }
        for source in bytes.chunks_exact(row) {
            if deep {
                for (target, sample) in current
                    .as_chunks_mut::<2>()
                    .0
                    .iter_mut()
                    .zip(source.as_chunks::<2>().0)
                {
                    *target = u16::from_ne_bytes(*sample).to_be_bytes();
                }
            } else {
                current.copy_from_slice(source);
            }
            let mut best = (u64::MAX, 0);
            for (index, &filter) in FILTERS.iter().enumerate() {
                let candidate = &mut self.candidates[index];
                apply(filter, stride, current, previous, candidate);
                let cost = candidate
                    .iter()
                    .map(|&byte| u64::from((byte as i8).unsigned_abs()))
                    .sum();
                if cost < best.0 {
                    best = (cost, index);
                }
            }
            self.filtered.push(FILTERS[best.1]);
            self.filtered.extend_from_slice(&self.candidates[best.1]);
            std::mem::swap(current, previous);
        }
    }
}

fn apply(filter: u8, stride: usize, current: &[u8], previous: &[u8], out: &mut [u8]) {
    let (head, tail) = out.split_at_mut(stride.min(current.len()));
    let lefts = current.iter();
    let rest = current.iter().zip(previous).skip(head.len());
    match filter {
        SUB => {
            head.copy_from_slice(&current[..head.len()]);
            for ((target, (&value, _)), &left) in tail.iter_mut().zip(rest).zip(lefts) {
                *target = value.wrapping_sub(left);
            }
        }
        UP => {
            for (target, (&value, &up)) in out.iter_mut().zip(current.iter().zip(previous)) {
                *target = value.wrapping_sub(up);
            }
        }
        AVERAGE => {
            for (target, (&value, &up)) in head.iter_mut().zip(current.iter().zip(previous)) {
                *target = value.wrapping_sub(up / 2);
            }
            for ((target, (&value, &up)), &left) in tail.iter_mut().zip(rest).zip(lefts) {
                let average = ((u16::from(left) + u16::from(up)) / 2) as u8;
                *target = value.wrapping_sub(average);
            }
        }
        PAETH => {
            for (target, (&value, &up)) in head.iter_mut().zip(current.iter().zip(previous)) {
                *target = value.wrapping_sub(up);
            }
            let corners = lefts.zip(previous);
            for ((target, (&value, &up)), (&left, &up_left)) in
                tail.iter_mut().zip(rest).zip(corners)
            {
                *target = value.wrapping_sub(predictor(left, up, up_left));
            }
        }
        _ => out.copy_from_slice(current),
    }
}
