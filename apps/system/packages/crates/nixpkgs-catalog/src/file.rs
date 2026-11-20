use std::fs::File;
use std::io::{self, Read, Write};
use std::os::unix::fs::FileExt;
use std::path::Path;

use rikka_file_format::{BinaryFormat, DecodeError, Reader, Writer};

use crate::error::{Error, Result};
use crate::import::Imported;

const FORMAT: BinaryFormat = BinaryFormat::new("package catalog", *b"RKNIXPKG");
const PREFIX_LEN: usize = BinaryFormat::HEADER_LEN + 8;
const RECORDS_PER_BLOCK: usize = 64;
const ZSTD_LEVEL: i32 = 9;
const SPAN_BYTES: usize = 29;
const BLOCK_BYTES: usize = 16;
const TEXT_BYTES: usize = 4;
const LARGEST_BLOCK: u32 = 1 << 24;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Span {
    pub(crate) ends: [u32; 4],
    pub(crate) unfree: bool,
    pub(crate) detail: DetailLocation,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DetailLocation {
    block: u32,
    at: u32,
    length: u32,
}

#[derive(Clone, Copy, Debug)]
struct Block {
    at: u64,
    packed: u32,
    unpacked: u32,
}

pub(crate) struct Contents {
    pub(crate) release: String,
    pub(crate) platforms: Vec<String>,
    pub(crate) text: String,
    pub(crate) spans: Vec<Span>,
    pub(crate) details: Details,
}

pub(crate) struct Details {
    file: File,
    at: u64,
    blocks: Vec<Block>,
}

impl Details {
    pub(crate) fn read(&self, location: DetailLocation) -> io::Result<Vec<u8>> {
        let block = self.blocks[location.block as usize];
        let mut packed = vec![0u8; block.packed as usize];
        self.file.read_exact_at(&mut packed, self.at + block.at)?;
        let unpacked = zstd::bulk::decompress(&packed, block.unpacked as usize)?;
        let (from, to) = (
            location.at as usize,
            location.at as usize + location.length as usize,
        );
        unpacked
            .get(from..to)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| corrupt(DecodeError::Malformed("detail out of block bounds")))
    }
}

pub(crate) fn write(path: &Path, release: &str, imported: &Imported) -> Result<()> {
    let write_error = Error::write(path);
    let mut file = rikka_fs::Replace::new(path).stage().map_err(&write_error)?;
    write_contents(&mut file, release, imported).map_err(&write_error)?;
    file.commit().map_err(&write_error)
}

fn write_contents(out: &mut impl Write, release: &str, imported: &Imported) -> io::Result<()> {
    let mut joined = String::new();
    let mut spans = Vec::with_capacity(imported.records.len());
    let mut blocks = Vec::new();
    let mut packed = Vec::new();
    for (index, chunk) in imported.records.chunks(RECORDS_PER_BLOCK).enumerate() {
        let mut unpacked = Vec::new();
        for record in chunk {
            let mut ends = [0u32; 4];
            for (end, field) in ends.iter_mut().zip([
                &record.attribute,
                &record.name,
                &record.version,
                &record.description,
            ]) {
                joined.push_str(field);
                *end = to_u32(joined.len())?;
            }
            let detail = &imported.details[record.detail.clone()];
            spans.push(Span {
                ends,
                unfree: record.unfree,
                detail: DetailLocation {
                    block: to_u32(index)?,
                    at: to_u32(unpacked.len())?,
                    length: to_u32(detail.len())?,
                },
            });
            unpacked.extend_from_slice(detail);
        }
        let compressed = zstd::bulk::compress(&unpacked, ZSTD_LEVEL)?;
        blocks.push(Block {
            at: packed.len() as u64,
            packed: to_u32(compressed.len())?,
            unpacked: to_u32(unpacked.len())?,
        });
        packed.extend_from_slice(&compressed);
    }

    let mut index = Vec::new();
    let mut writer = Writer::new(&mut index);
    writer.str(release);
    writer.count32(imported.platforms.len());
    for platform in &imported.platforms {
        writer.str(platform);
    }
    writer.count32(spans.len());
    writer.str(&joined);
    for span in &spans {
        writer.put_all(&span.ends);
        writer.put(&span.unfree);
        writer.put_all(&[span.detail.block, span.detail.at, span.detail.length]);
    }
    writer.count32(blocks.len());
    for block in &blocks {
        writer.put(&block.at);
        writer.put(&block.packed);
        writer.put(&block.unpacked);
    }

    out.write_all(&FORMAT.header())?;
    out.write_all(&(index.len() as u64).to_le_bytes())?;
    out.write_all(&index)?;
    out.write_all(&packed)
}

fn to_u32(length: usize) -> io::Result<u32> {
    u32::try_from(length)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "length exceeds u32 range"))
}

pub(crate) fn read(path: &Path) -> Result<Contents> {
    let read_error = Error::read(path);
    let mut file = File::open(path).map_err(&read_error)?;
    let size = file.metadata().map_err(&read_error)?.len();
    let mut prefix = [0u8; PREFIX_LEN];
    file.read_exact(&mut prefix).map_err(&read_error)?;
    let mut head = Reader::new(&prefix);
    FORMAT
        .read_header(&mut head)
        .map_err(|error| read_error(corrupt(error)))?;
    let index_len = head
        .get::<u64>()
        .map_err(|error| read_error(corrupt(error)))?;
    let at = (PREFIX_LEN as u64)
        .checked_add(index_len)
        .filter(|&at| at <= size)
        .ok_or_else(|| read_error(corrupt(DecodeError::Malformed("index overruns the file"))))?;
    let mut index = vec![0u8; index_len as usize];
    file.read_exact(&mut index).map_err(&read_error)?;
    decode_index(&index, file, at, size - at).map_err(|error| read_error(corrupt(error)))
}

fn decode_index(index: &[u8], file: File, at: u64, length: u64) -> Result<Contents, DecodeError> {
    let mut input = Reader::new(index);
    let release = input.str()?.to_owned();
    let platforms = (0..input.count32(TEXT_BYTES)?)
        .map(|_| input.str().map(str::to_owned))
        .collect::<Result<Vec<String>, _>>()?;
    let count = input.count32(SPAN_BYTES)?;
    let text = input.str()?.to_owned();

    let mut spans = Vec::with_capacity(count);
    let mut last = 0u32;
    for _ in 0..count {
        let mut ends = [0u32; 4];
        for end in &mut ends {
            *end = input.get()?;
            if *end < last || !text.is_char_boundary(*end as usize) {
                return Err(DecodeError::Malformed("field offset out of bounds"));
            }
            last = *end;
        }
        spans.push(Span {
            ends,
            unfree: input.get()?,
            detail: DetailLocation {
                block: input.get()?,
                at: input.get()?,
                length: input.get()?,
            },
        });
    }

    let blocks = (0..input.count32(BLOCK_BYTES)?)
        .map(|_| {
            Ok(Block {
                at: input.get()?,
                packed: input.get()?,
                unpacked: input.get()?,
            })
        })
        .collect::<Result<Vec<Block>, DecodeError>>()?;
    input.finish()?;
    if blocks.iter().any(|block| {
        block.unpacked > LARGEST_BLOCK || block.at.saturating_add(u64::from(block.packed)) > length
    }) {
        return Err(DecodeError::Malformed("block out of file bounds"));
    }
    if spans.iter().any(|span| {
        blocks.get(span.detail.block as usize).is_none_or(|block| {
            u64::from(span.detail.at) + u64::from(span.detail.length) > u64::from(block.unpacked)
        })
    }) {
        return Err(DecodeError::Malformed("detail out of block bounds"));
    }

    Ok(Contents {
        release,
        platforms,
        text,
        spans,
        details: Details { file, at, blocks },
    })
}

fn corrupt(error: DecodeError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
