use crate::blend::BlendMode;
use crate::descriptor::{Descriptor, read_versioned, write_versioned};
use crate::error::Result;
use crate::io::{Padding, Reader, Width, Writer};
use crate::key::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Other,
    OpenFolder,
    ClosedFolder,
    BoundingDivider,
}

impl SectionKind {
    pub(crate) fn from_code(code: u32) -> Self {
        match code {
            1 => SectionKind::OpenFolder,
            2 => SectionKind::ClosedFolder,
            3 => SectionKind::BoundingDivider,
            _ => SectionKind::Other,
        }
    }

    pub(crate) fn code(self) -> u32 {
        match self {
            SectionKind::Other => 0,
            SectionKind::OpenFolder => 1,
            SectionKind::ClosedFolder => 2,
            SectionKind::BoundingDivider => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Section {
    pub kind: SectionKind,
    pub blend_mode: Option<BlendMode>,
    pub sub_kind: Option<u32>,
    pub nested: bool,
}

impl Section {
    pub fn new(kind: SectionKind) -> Self {
        Self {
            kind,
            blend_mode: None,
            sub_kind: None,
            nested: false,
        }
    }

    pub(crate) fn read(reader: &mut Reader<'_>, nested: bool) -> Result<Self> {
        let kind = SectionKind::from_code(reader.u32()?);
        let mut blend_mode = None;
        let mut sub_kind = None;
        if reader.remaining() >= 8 {
            reader.expect(b"8BIM")?;
            blend_mode = Some(BlendMode::from_key(reader.key()?));
            if reader.remaining() >= 4 {
                sub_kind = Some(reader.u32()?);
            }
        }
        Ok(Self {
            kind,
            blend_mode,
            sub_kind,
            nested,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) {
        writer.u32(self.kind.code());
        if let Some(mode) = self.blend_mode {
            writer.code(b"8BIM");
            writer.key(mode.key());
            if let Some(sub) = self.sub_kind {
                writer.u32(sub);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetColor {
    None,
    Red,
    Orange,
    Yellow,
    Green,
    Blue,
    Violet,
    Gray,
    Other(u16),
}

impl SheetColor {
    pub(crate) fn from_code(code: u16) -> Self {
        match code {
            0 => SheetColor::None,
            1 => SheetColor::Red,
            2 => SheetColor::Orange,
            3 => SheetColor::Yellow,
            4 => SheetColor::Green,
            5 => SheetColor::Blue,
            6 => SheetColor::Violet,
            7 => SheetColor::Gray,
            other => SheetColor::Other(other),
        }
    }

    pub(crate) fn code(self) -> u16 {
        match self {
            SheetColor::None => 0,
            SheetColor::Red => 1,
            SheetColor::Orange => 2,
            SheetColor::Yellow => 3,
            SheetColor::Green => 4,
            SheetColor::Blue => 5,
            SheetColor::Violet => 6,
            SheetColor::Gray => 7,
            SheetColor::Other(code) => code,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knockout {
    None,
    Shallow,
    Deep,
}

impl Knockout {
    pub(crate) fn from_code(code: u8) -> Self {
        match code {
            0 => Knockout::None,
            2 => Knockout::Deep,
            _ => Knockout::Shallow,
        }
    }

    pub(crate) fn code(self) -> u8 {
        match self {
            Knockout::None => 0,
            Knockout::Shallow => 1,
            Knockout::Deep => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Protection(pub u32);

impl Protection {
    pub fn transparency(self) -> bool {
        self.0 & 0x01 != 0
    }

    pub fn composite(self) -> bool {
        self.0 & 0x02 != 0
    }

    pub fn position(self) -> bool {
        self.0 & 0x04 != 0
    }

    pub fn artboards(self) -> bool {
        self.0 & 0x08 != 0
    }

    pub fn all(self) -> bool {
        self.0 & 0x8000_0000 != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MetadataValue {
    Descriptor(Descriptor),
    Integer(u32),
    Raw(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetadataItem {
    pub signature: Key,
    pub key: Key,
    pub copy_on_duplicate: bool,
    pub value: MetadataValue,
}

pub(crate) fn read_metadata(reader: &mut Reader<'_>) -> Result<Vec<MetadataItem>> {
    let count = reader.u32()?;
    let mut items = Vec::with_capacity(count.min(256) as usize);
    for _ in 0..count {
        let signature = reader.key()?;
        let key = reader.key()?;
        let copy_on_duplicate = reader.bool()?;
        reader.skip(3)?;
        let length = u64::from(reader.u32()?);
        let mut body = reader.sub(length)?;
        let value = if matches!(&key.0, b"mdyn" | b"sgrp") && body.remaining() >= 4 {
            MetadataValue::Integer(body.u32()?)
        } else {
            let raw = body.rest().to_vec();
            match read_versioned(&mut body) {
                Ok(descriptor) if body.finish_padded() => MetadataValue::Descriptor(descriptor),
                _ => MetadataValue::Raw(raw),
            }
        };
        items.push(MetadataItem {
            signature,
            key,
            copy_on_duplicate,
            value,
        });
    }
    Ok(items)
}

pub(crate) fn write_metadata(writer: &mut Writer, items: &[MetadataItem]) -> Result<()> {
    writer.u32(items.len() as u32);
    for item in items {
        writer.key(item.signature);
        writer.key(item.key);
        writer.bool(item.copy_on_duplicate);
        writer.zeros(3);
        writer.section(Width::Four, Padding::Counted(4), |writer| {
            match &item.value {
                MetadataValue::Descriptor(descriptor) => write_versioned(writer, descriptor),
                MetadataValue::Integer(value) => {
                    writer.u32(*value);
                    Ok(())
                }
                MetadataValue::Raw(bytes) => {
                    writer.bytes(bytes);
                    Ok(())
                }
            }
        })?;
    }
    Ok(())
}
