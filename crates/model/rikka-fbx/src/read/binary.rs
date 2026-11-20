use std::io::Read;
use std::sync::Arc;

use flate2::read::ZlibDecoder;

use crate::document::{Node, Value};
use rikka_model_core::{ByteReader, ImportCtx};
use rikka_model_core::{Error, Result};

pub(crate) const MAGIC: &[u8] = b"Kaydara FBX Binary  \x00";

const HEADER: usize = 27;
const WIDE_VERSION: u32 = 7500;
const MAX_DEPTH: usize = 64;

pub(crate) fn parse(bytes: &[u8], ctx: &mut ImportCtx<'_>) -> Result<(u32, Vec<Node>)> {
    if bytes.len() < HEADER || !bytes.starts_with(MAGIC) {
        return Err(Error::Damaged("missing binary FBX magic".to_owned()));
    }
    let version = u32::from_le_bytes([bytes[23], bytes[24], bytes[25], bytes[26]]);
    let mut cursor = ByteReader::new(bytes);
    cursor.seek(HEADER)?;
    let mut reader = Reader {
        cursor,
        length: bytes.len(),
        wide: version >= WIDE_VERSION,
    };
    let nodes = reader.children(bytes.len(), 0, ctx)?;
    Ok((version, nodes))
}

struct Reader<'a> {
    cursor: ByteReader<'a>,
    length: usize,
    wide: bool,
}

impl Reader<'_> {
    fn children(&mut self, end: usize, depth: usize, ctx: &mut ImportCtx<'_>) -> Result<Vec<Node>> {
        if depth > MAX_DEPTH {
            return Err(Error::Damaged(format!(
                "records nest deeper than {MAX_DEPTH}"
            )));
        }
        let mut nodes = Vec::new();
        while self.cursor.position() + self.sentinel() <= end {
            match self.node(depth, ctx)? {
                Some(node) => nodes.push(node),
                None => break,
            }
        }
        Ok(nodes)
    }

    fn node(&mut self, depth: usize, ctx: &mut ImportCtx<'_>) -> Result<Option<Node>> {
        ctx.checkpoint()?;
        let (end_offset, value_count, value_bytes) = match self.wide {
            true => (self.cursor.u64()?, self.cursor.u64()?, self.cursor.u64()?),
            false => (
                u64::from(self.cursor.u32()?),
                u64::from(self.cursor.u32()?),
                u64::from(self.cursor.u32()?),
            ),
        };
        let name_length = usize::from(self.cursor.u8()?);
        if end_offset == 0 && name_length == 0 {
            return Ok(None);
        }
        let end = usize::try_from(end_offset).unwrap_or(usize::MAX);
        if end > self.length || end < self.cursor.position() {
            return Err(Error::Damaged(format!(
                "record end offset {end_offset} exceeds {} byte document",
                self.length
            )));
        }

        let name = String::from_utf8_lossy(self.cursor.take(name_length)?).into_owned();
        let values_end = self
            .cursor
            .position()
            .saturating_add(usize::try_from(value_bytes).unwrap_or(0));
        let mut values = Vec::with_capacity(usize::try_from(value_count).unwrap_or(0).min(16));
        for _ in 0..value_count {
            values.push(self.value(ctx)?);
        }
        self.cursor
            .seek(values_end.clamp(self.cursor.position(), end))?;

        let children = match self.cursor.position() < end {
            true => self.children(end, depth + 1, ctx)?,
            false => Vec::new(),
        };
        self.cursor.seek(end)?;
        Ok(Some(Node {
            name,
            values,
            children,
        }))
    }

    fn value(&mut self, ctx: &mut ImportCtx<'_>) -> Result<Value> {
        let kind = self.cursor.u8()?;
        match kind {
            b'Y' => Ok(Value::I16(self.cursor.i16()?)),
            b'C' => Ok(Value::Bool(self.cursor.u8()? != 0)),
            b'I' => Ok(Value::I32(self.cursor.u32()? as i32)),
            b'F' => Ok(Value::F32(f32::from_bits(self.cursor.u32()?))),
            b'D' => Ok(Value::F64(f64::from_bits(self.cursor.u64()?))),
            b'L' => Ok(Value::I64(self.cursor.u64()? as i64)),
            b'S' => {
                let length = self.cursor.u32()? as usize;
                let bytes = self.cursor.take(length)?;
                Ok(Value::Text(String::from_utf8_lossy(bytes).into_owned()))
            }
            b'R' => {
                let length = self.cursor.u32()? as usize;
                Ok(Value::Raw(Arc::from(self.cursor.take(length)?)))
            }
            b'b' | b'c' | b'i' | b'l' | b'f' | b'd' => self.array(kind, ctx),
            other => Err(Error::Damaged(format!(
                "unknown property type '{}'",
                char::from(other)
            ))),
        }
    }

    fn array(&mut self, kind: u8, ctx: &mut ImportCtx<'_>) -> Result<Value> {
        let count = self.cursor.u32()? as usize;
        let encoding = self.cursor.u32()?;
        let encoded_length = self.cursor.u32()? as usize;
        let width = match kind {
            b'b' | b'c' => 1,
            b'i' | b'f' => 4,
            _ => 8,
        };
        let decoded = count
            .checked_mul(width)
            .ok_or_else(|| Error::Damaged("array length exceeds data".to_owned()))?;
        ctx.charge(decoded as u64)?;

        let encoded = self.cursor.take(encoded_length)?;
        let data = match encoding {
            0 => match encoded.len() >= decoded {
                true => std::borrow::Cow::Borrowed(&encoded[..decoded]),
                false => {
                    return Err(Error::Damaged(format!(
                        "array of {count} values has {} bytes",
                        encoded.len()
                    )));
                }
            },
            1 => std::borrow::Cow::Owned(inflate(encoded, decoded)?),
            other => {
                return Err(Error::Damaged(format!("unknown array encoding {other}")));
            }
        };

        Ok(match kind {
            b'b' | b'c' => Value::Bools(data.iter().map(|&byte| byte != 0).collect()),
            b'i' => Value::I32s(chunks(&data).map(i32::from_le_bytes).collect()),
            b'f' => Value::F32s(chunks(&data).map(f32::from_le_bytes).collect()),
            b'l' => Value::I64s(chunks(&data).map(i64::from_le_bytes).collect()),
            _ => Value::F64s(chunks(&data).map(f64::from_le_bytes).collect()),
        })
    }

    fn sentinel(&self) -> usize {
        match self.wide {
            true => 25,
            false => 13,
        }
    }
}

fn chunks<const N: usize>(bytes: &[u8]) -> impl Iterator<Item = [u8; N]> + '_ {
    bytes.as_chunks::<N>().0.iter().copied()
}

fn inflate(encoded: &[u8], expected: usize) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(expected);
    ZlibDecoder::new(encoded)
        .take(expected as u64)
        .read_to_end(&mut out)
        .map_err(|error| Error::Damaged(format!("failed to inflate array: {error}")))?;
    match out.len() == expected {
        true => Ok(out),
        false => Err(Error::Damaged(format!(
            "inflated array has {} bytes, expected {expected}",
            out.len()
        ))),
    }
}
