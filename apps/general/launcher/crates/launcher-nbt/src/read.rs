use crate::error::{Error, Result};
use crate::mutf8;
use crate::tag::{Compound, Kind, List, Tag};

const MAX_DEPTH: usize = 512;

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        if self.bytes.len() < count {
            return Err(Error::Truncated);
        }
        let (taken, rest) = self.bytes.split_at(count);
        self.bytes = rest;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.take(N)?.try_into().expect("take returns N bytes"))
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize> {
        let length = i32::from_be_bytes(self.array()?);
        let length = usize::try_from(length).map_err(|_| Error::Length(length))?;
        if length > self.bytes.len() {
            return Err(Error::Truncated);
        }
        Ok(length)
    }

    fn string(&mut self) -> Result<String> {
        let length = u16::from_be_bytes(self.array()?);
        mutf8::decode(self.take(usize::from(length))?)
    }

    fn kind(&mut self) -> Result<Kind> {
        let id = self.byte()?;
        Kind::from_id(id).ok_or(Error::Kind(id))
    }

    fn payload(&mut self, kind: Kind, depth: usize) -> Result<Tag> {
        if depth > MAX_DEPTH {
            return Err(Error::Depth(MAX_DEPTH));
        }
        Ok(match kind {
            Kind::End => return Err(Error::Kind(0)),
            Kind::Byte => Tag::Byte(i8::from_be_bytes(self.array()?)),
            Kind::Short => Tag::Short(i16::from_be_bytes(self.array()?)),
            Kind::Int => Tag::Int(i32::from_be_bytes(self.array()?)),
            Kind::Long => Tag::Long(i64::from_be_bytes(self.array()?)),
            Kind::Float => Tag::Float(f32::from_be_bytes(self.array()?)),
            Kind::Double => Tag::Double(f64::from_be_bytes(self.array()?)),
            Kind::ByteArray => {
                let length = self.length()?;
                Tag::ByteArray(
                    self.take(length)?
                        .iter()
                        .map(|byte| i8::from_be_bytes([*byte]))
                        .collect(),
                )
            }
            Kind::String => Tag::String(self.string()?),
            Kind::List => {
                let element = self.kind()?;
                let length = self.length()?;
                let mut items = Vec::with_capacity(length.min(4096));
                if element == Kind::End {
                    return Ok(Tag::List(List::empty()));
                }
                for _ in 0..length {
                    items.push(self.payload(element, depth + 1)?);
                }
                Tag::List(List::from_parts(element, items))
            }
            Kind::Compound => Tag::Compound(self.compound(depth + 1)?),
            Kind::IntArray => {
                let length = self.length()?;
                let mut values = Vec::with_capacity(length.min(4096));
                for _ in 0..length {
                    values.push(i32::from_be_bytes(self.array()?));
                }
                Tag::IntArray(values)
            }
            Kind::LongArray => {
                let length = self.length()?;
                let mut values = Vec::with_capacity(length.min(4096));
                for _ in 0..length {
                    values.push(i64::from_be_bytes(self.array()?));
                }
                Tag::LongArray(values)
            }
        })
    }

    fn compound(&mut self, depth: usize) -> Result<Compound> {
        let mut compound = Compound::new();
        loop {
            let kind = self.kind()?;
            if kind == Kind::End {
                return Ok(compound);
            }
            let name = self.string()?;
            let tag = self.payload(kind, depth)?;
            compound.insert(name, tag);
        }
    }
}

pub fn read(bytes: &[u8]) -> Result<(String, Compound)> {
    let mut reader = Reader { bytes };
    if reader.kind()? != Kind::Compound {
        return Err(Error::Root);
    }
    let name = reader.string()?;
    let root = reader.compound(1)?;
    Ok((name, root))
}
