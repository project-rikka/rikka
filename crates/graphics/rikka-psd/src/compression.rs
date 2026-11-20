use std::io::{Read, Write};

use crate::error::{Error, Result};
use crate::io::{Reader, Writer};
use crate::pixels::{Depth, Plane};

const MAX_DEFLATE_RATIO: usize = 1032;
const MAX_PACK_BITS_RATIO: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Compression {
    Raw,
    #[default]
    Rle,
    Zip,
    ZipPredicted,
}

impl Compression {
    pub fn from_code(code: u16) -> Option<Self> {
        match code {
            0 => Some(Compression::Raw),
            1 => Some(Compression::Rle),
            2 => Some(Compression::Zip),
            3 => Some(Compression::ZipPredicted),
            _ => None,
        }
    }

    pub fn code(self) -> u16 {
        match self {
            Compression::Raw => 0,
            Compression::Rle => 1,
            Compression::Zip => 2,
            Compression::ZipPredicted => 3,
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum Encoded {
    Raw(Vec<u8>),
    Rle { rows: Vec<u32>, packed: Vec<u8> },
    Zip(Vec<u8>),
    ZipPredicted(Vec<u8>),
}

impl std::fmt::Debug for Encoded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Encoded::{:?}({} bytes)",
            self.compression(),
            self.payload_len()
        )
    }
}

impl Default for Encoded {
    fn default() -> Self {
        Encoded::Raw(Vec::new())
    }
}

impl Encoded {
    pub fn compression(&self) -> Compression {
        match self {
            Encoded::Raw(_) => Compression::Raw,
            Encoded::Rle { .. } => Compression::Rle,
            Encoded::Zip(_) => Compression::Zip,
            Encoded::ZipPredicted(_) => Compression::ZipPredicted,
        }
    }

    fn payload_len(&self) -> usize {
        match self {
            Encoded::Raw(data) | Encoded::Zip(data) | Encoded::ZipPredicted(data) => data.len(),
            Encoded::Rle { packed, .. } => packed.len(),
        }
    }

    pub fn encode(plane: &Plane, compression: Compression) -> Encoded {
        let width = plane.width();
        let height = plane.height();
        let depth = plane.depth();
        let raw = plane.to_bytes();
        match compression {
            Compression::Raw => Encoded::Raw(raw),
            Compression::Rle => {
                let row_bytes = depth.row_bytes(width);
                let mut rows = Vec::with_capacity(height as usize);
                let mut packed = Vec::with_capacity(raw.len() / 2);
                if row_bytes > 0 {
                    for row in raw.chunks_exact(row_bytes) {
                        let before = packed.len();
                        pack_bits(row, &mut packed);
                        rows.push((packed.len() - before) as u32);
                    }
                }
                Encoded::Rle { rows, packed }
            }
            Compression::Zip => Encoded::Zip(deflate(&raw)),
            Compression::ZipPredicted => {
                let mut data = raw;
                predict(&mut data, width, depth);
                Encoded::ZipPredicted(deflate(&data))
            }
        }
    }

    pub fn decode(&self, width: u32, height: u32, depth: Depth) -> Result<Plane> {
        let expected =
            usize::try_from(depth.plane_bytes(width, height)).map_err(|_| Error::Oversized)?;
        let raw = self.decode_bytes(width, height, depth, expected)?;
        Plane::from_bytes(width, height, depth, &raw)
    }

    pub(crate) fn decode_bytes(
        &self,
        width: u32,
        height: u32,
        depth: Depth,
        expected: usize,
    ) -> Result<Vec<u8>> {
        let row_bytes = depth.row_bytes(width);
        if self.max_decoded_len().saturating_add(row_bytes) < expected {
            return Err(Error::CorruptChannel);
        }
        let mut raw = match self {
            Encoded::Raw(data) => data.clone(),
            Encoded::Rle { rows, packed } => {
                let mut out = Vec::with_capacity(expected);
                let mut offset = 0usize;
                for &count in rows.iter().take(height as usize) {
                    let end = (offset + count as usize).min(packed.len());
                    let start = out.len();
                    unpack_bits(&packed[offset.min(end)..end], &mut out, row_bytes);
                    out.resize(start + row_bytes, 0);
                    offset = end;
                }
                out
            }
            Encoded::Zip(data) => inflate(data, expected)?,
            Encoded::ZipPredicted(data) => {
                let mut out = inflate(data, expected)?;
                out.resize(expected, 0);
                unpredict(&mut out, width, depth);
                out
            }
        };
        if raw.len() < expected
            && !matches!(self, Encoded::Raw(_))
            && raw.len() + row_bytes < expected
        {
            return Err(Error::CorruptChannel);
        }
        raw.resize(expected, 0);
        Ok(raw)
    }

    fn max_decoded_len(&self) -> usize {
        match self {
            Encoded::Raw(_) => usize::MAX,
            Encoded::Rle { packed, .. } => packed.len().saturating_mul(MAX_PACK_BITS_RATIO),
            Encoded::Zip(data) | Encoded::ZipPredicted(data) => {
                data.len().saturating_mul(MAX_DEFLATE_RATIO)
            }
        }
    }

    pub(crate) fn read_channel(
        reader: &mut Reader<'_>,
        compression: Compression,
        height: u32,
        wide: bool,
    ) -> Result<Encoded> {
        Ok(match compression {
            Compression::Raw => Encoded::Raw(reader.take_rest().to_vec()),
            Compression::Rle => Encoded::Rle {
                rows: read_counts(reader, height, wide)?,
                packed: reader.take_rest().to_vec(),
            },
            Compression::Zip => Encoded::Zip(reader.take_rest().to_vec()),
            Compression::ZipPredicted => Encoded::ZipPredicted(reader.take_rest().to_vec()),
        })
    }

    pub(crate) fn write_body(&self, writer: &mut Writer, wide: bool) -> Result<()> {
        match self {
            Encoded::Raw(data) | Encoded::Zip(data) | Encoded::ZipPredicted(data) => {
                writer.bytes(data)
            }
            Encoded::Rle { rows, packed } => {
                write_counts(writer, rows, wide)?;
                writer.bytes(packed);
            }
        }
        Ok(())
    }

    pub(crate) fn file_len(&self, wide: bool) -> u64 {
        let counts = match self {
            Encoded::Rle { rows, .. } => rows.len() * if wide { 4 } else { 2 },
            _ => 0,
        };
        (2 + counts + self.payload_len()) as u64
    }

    pub(crate) fn read_composite(
        reader: &mut Reader<'_>,
        channels: usize,
        width: u32,
        height: u32,
        depth: Depth,
        wide: bool,
    ) -> Result<Vec<Encoded>> {
        let code = reader.u16()?;
        let compression = Compression::from_code(code).ok_or(Error::Invalid {
            what: "composite compression",
            value: i64::from(code),
        })?;
        let plane_bytes =
            usize::try_from(depth.plane_bytes(width, height)).map_err(|_| Error::Oversized)?;
        match compression {
            Compression::Raw => (0..channels)
                .map(|_| {
                    let take = plane_bytes.min(reader.remaining());
                    Ok(Encoded::Raw(reader.bytes(take)?.to_vec()))
                })
                .collect(),
            Compression::Rle => {
                let mut counts = Vec::with_capacity(channels);
                for _ in 0..channels {
                    counts.push(read_counts(reader, height, wide)?);
                }
                counts
                    .into_iter()
                    .map(|rows| {
                        let total: u64 = rows.iter().map(|&r| u64::from(r)).sum();
                        let take = usize::try_from(total)
                            .map_err(|_| Error::Oversized)?
                            .min(reader.remaining());
                        Ok(Encoded::Rle {
                            rows,
                            packed: reader.bytes(take)?.to_vec(),
                        })
                    })
                    .collect()
            }
            Compression::Zip | Compression::ZipPredicted => {
                let all = inflate(reader.take_rest(), plane_bytes * channels)?;
                Ok(all
                    .chunks(plane_bytes.max(1))
                    .take(channels)
                    .map(|chunk| {
                        let mut data = chunk.to_vec();
                        data.resize(plane_bytes, 0);
                        if compression == Compression::ZipPredicted {
                            unpredict(&mut data, width, depth);
                        }
                        Encoded::Raw(data)
                    })
                    .collect())
            }
        }
    }

    pub(crate) fn write_composite(
        writer: &mut Writer,
        planes: &[Encoded],
        wide: bool,
    ) -> Result<()> {
        let compression = planes
            .first()
            .map_or(Compression::Raw, Encoded::compression);
        writer.u16(compression.code());
        match compression {
            Compression::Rle => {
                for plane in planes {
                    if let Encoded::Rle { rows, .. } = plane {
                        write_counts(writer, rows, wide)?;
                    }
                }
                for plane in planes {
                    if let Encoded::Rle { packed, .. } = plane {
                        writer.bytes(packed);
                    }
                }
            }
            _ => {
                for plane in planes {
                    if let Encoded::Raw(data) = plane {
                        writer.bytes(data);
                    }
                }
            }
        }
        Ok(())
    }
}

fn read_counts(reader: &mut Reader<'_>, height: u32, wide: bool) -> Result<Vec<u32>> {
    let width = if wide { 4 } else { 2 };
    let mut rows = Vec::with_capacity((height as usize).min(reader.remaining() / width));
    for _ in 0..height {
        rows.push(if wide {
            reader.u32()?
        } else {
            u32::from(reader.u16()?)
        });
    }
    Ok(rows)
}

fn write_counts(writer: &mut Writer, rows: &[u32], wide: bool) -> Result<()> {
    for &count in rows {
        if wide {
            writer.u32(count);
        } else {
            writer.u16(u16::try_from(count).map_err(|_| Error::Oversized)?);
        }
    }
    Ok(())
}

pub(crate) fn pack_bits(row: &[u8], out: &mut Vec<u8>) {
    let mut index = 0;
    let length = row.len();
    while index < length {
        let mut run = 1;
        while index + run < length && run < 128 && row[index + run] == row[index] {
            run += 1;
        }
        if run >= 3 || (run == 2 && index + run == length) {
            out.push((257 - run) as u8);
            out.push(row[index]);
            index += run;
            continue;
        }
        let start = index;
        let mut literal = 0;
        while index < length && literal < 128 {
            if index + 2 < length && row[index] == row[index + 1] && row[index] == row[index + 2] {
                break;
            }
            index += 1;
            literal += 1;
        }
        out.push((literal - 1) as u8);
        out.extend_from_slice(&row[start..start + literal]);
    }
}

pub(crate) fn unpack_bits(packed: &[u8], out: &mut Vec<u8>, row_bytes: usize) {
    let limit = out.len() + row_bytes;
    let mut index = 0;
    while index < packed.len() && out.len() < limit {
        let header = packed[index] as i8;
        index += 1;
        if header >= 0 {
            let count = header as usize + 1;
            let end = (index + count).min(packed.len());
            let take = (end - index).min(limit - out.len());
            out.extend_from_slice(&packed[index..index + take]);
            index = end;
        } else if header != -128 {
            let count = (1 - i32::from(header)) as usize;
            let Some(&value) = packed.get(index) else {
                break;
            };
            index += 1;
            let take = count.min(limit - out.len());
            out.resize(out.len() + take, value);
        }
    }
}

pub(crate) fn deflate(data: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(
        Vec::with_capacity(data.len() / 2),
        flate2::Compression::default(),
    );
    encoder.write_all(data).expect("in-memory write");
    encoder.finish().expect("in-memory write")
}

pub(crate) fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(limit.min(data.len().saturating_mul(MAX_DEFLATE_RATIO)));
    let decoder = flate2::read::ZlibDecoder::new(data);
    decoder
        .take(limit as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|_| Error::CorruptChannel)?;
    if out.len() > limit {
        return Err(Error::CorruptChannel);
    }
    Ok(out)
}

fn predict(data: &mut [u8], width: u32, depth: Depth) {
    let width = width as usize;
    match depth {
        Depth::One | Depth::Eight => {
            for row in data.chunks_exact_mut(width.max(1)) {
                for x in (1..row.len()).rev() {
                    row[x] = row[x].wrapping_sub(row[x - 1]);
                }
            }
        }
        Depth::Sixteen => {
            for row in data.chunks_exact_mut((width * 2).max(1)) {
                for x in (1..width).rev() {
                    let current = u16::from_be_bytes([row[x * 2], row[x * 2 + 1]]);
                    let previous = u16::from_be_bytes([row[x * 2 - 2], row[x * 2 - 1]]);
                    row[x * 2..x * 2 + 2]
                        .copy_from_slice(&current.wrapping_sub(previous).to_be_bytes());
                }
            }
        }
        Depth::ThirtyTwo => {
            let mut planar = vec![0u8; width * 4];
            for row in data.chunks_exact_mut((width * 4).max(1)) {
                for x in 0..width {
                    for byte in 0..4 {
                        planar[byte * width + x] = row[x * 4 + byte];
                    }
                }
                for x in (1..planar.len()).rev() {
                    planar[x] = planar[x].wrapping_sub(planar[x - 1]);
                }
                row.copy_from_slice(&planar);
            }
        }
    }
}

fn unpredict(data: &mut [u8], width: u32, depth: Depth) {
    let width = width as usize;
    match depth {
        Depth::One | Depth::Eight => {
            for row in data.chunks_exact_mut(width.max(1)) {
                for x in 1..row.len() {
                    row[x] = row[x].wrapping_add(row[x - 1]);
                }
            }
        }
        Depth::Sixteen => {
            for row in data.chunks_exact_mut((width * 2).max(1)) {
                for x in 1..width {
                    let current = u16::from_be_bytes([row[x * 2], row[x * 2 + 1]]);
                    let previous = u16::from_be_bytes([row[x * 2 - 2], row[x * 2 - 1]]);
                    row[x * 2..x * 2 + 2]
                        .copy_from_slice(&current.wrapping_add(previous).to_be_bytes());
                }
            }
        }
        Depth::ThirtyTwo => {
            let mut interleaved = vec![0u8; width * 4];
            for row in data.chunks_exact_mut((width * 4).max(1)) {
                for x in 1..row.len() {
                    row[x] = row[x].wrapping_add(row[x - 1]);
                }
                for x in 0..width {
                    for byte in 0..4 {
                        interleaved[x * 4 + byte] = row[byte * width + x];
                    }
                }
                row.copy_from_slice(&interleaved);
            }
        }
    }
}
