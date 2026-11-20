use crate::descriptor::{Descriptor, read_versioned, write_versioned};
use crate::error::Result;
use crate::io::{Reader, Writer};
use crate::key::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Header {
    None,
    U16(u16),
    U32(u32),
    Key(Key),
    KeyVersion(Key, u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    None,
    U16,
    U32,
    Key,
    KeyVersion,
}

fn shape(key: Key) -> Option<(Shape, Header)> {
    Some(match &key.0 {
        b"SoCo" | b"GdFl" | b"PtFl" | b"vstk" | b"artb" | b"artd" | b"abdd" | b"pths" | b"cinf"
        | b"extn" | b"extd" | b"CgEd" | b"PxSc" | b"vibA" | b"blwh" | b"anFX" | b"frgb"
        | b"FELS" | b"GenI" | b"OCIO" | b"shpa" => (Shape::None, Header::None),
        b"vscg" => (Shape::Key, Header::Key(Key(*b"SoCo"))),
        b"vogk" => (Shape::U32, Header::U32(1)),
        b"lfx2" | b"lfxs" | b"lmfx" => (Shape::U32, Header::U32(0)),
        b"clrL" => (Shape::U16, Header::U16(1)),
        b"CAI " => (Shape::U32, Header::U32(3)),
        b"SoLd" | b"SoLE" => (Shape::KeyVersion, Header::KeyVersion(Key(*b"soLD"), 4)),
        _ => return None,
    })
}

pub(crate) fn is_descriptor_key(key: Key) -> bool {
    shape(key).is_some()
}

#[derive(Debug, Clone, PartialEq)]
pub struct DescriptorBlock {
    pub key: Key,
    pub header: Header,
    pub descriptor: Descriptor,
}

impl DescriptorBlock {
    pub fn new(key: Key, descriptor: Descriptor) -> Self {
        let header = shape(key).map_or(Header::None, |(_, header)| header);
        Self {
            key,
            header,
            descriptor,
        }
    }

    pub(crate) fn read(key: Key, reader: &mut Reader<'_>) -> Result<Self> {
        let (shape, _) = shape(key).ok_or_else(|| reader.malformed("descriptor block"))?;
        let header = match shape {
            Shape::None => Header::None,
            Shape::U16 => Header::U16(reader.u16()?),
            Shape::U32 => Header::U32(reader.u32()?),
            Shape::Key => Header::Key(reader.key()?),
            Shape::KeyVersion => {
                let tag = reader.key()?;
                Header::KeyVersion(tag, reader.u32()?)
            }
        };
        let descriptor = read_versioned(reader)?;
        Ok(Self {
            key,
            header,
            descriptor,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) -> Result<()> {
        match self.header {
            Header::None => {}
            Header::U16(value) => writer.u16(value),
            Header::U32(value) => writer.u32(value),
            Header::Key(tag) => writer.key(tag),
            Header::KeyVersion(tag, version) => {
                writer.key(tag);
                writer.u32(version);
            }
        }
        write_versioned(writer, &self.descriptor)
    }
}
