use crate::color::Color;
use crate::error::Result;
use crate::geometry::Rect;
use crate::io::{Padding, Reader, Width, Writer};
use crate::key::Key;

#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationContent {
    Text(String),
    Sound(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub kind: Key,
    pub open: bool,
    pub flags: u8,
    pub optional_blocks: u16,
    pub icon: Rect,
    pub popup: Rect,
    pub color: Color,
    pub author: String,
    pub name: String,
    pub modified: String,
    pub marker: Key,
    pub content: AnnotationContent,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotations {
    pub major_version: u16,
    pub minor_version: u16,
    pub items: Vec<Annotation>,
}

impl Annotations {
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let major_version = reader.u16()?;
        let minor_version = reader.u16()?;
        let count = reader.u32()?;
        let mut items = Vec::with_capacity(count.min(256) as usize);
        for _ in 0..count {
            let length = reader.u32()?;
            let mut body = reader.sub(u64::from(length.saturating_sub(4)))?;
            items.push(read_annotation(&mut body)?);
        }
        Ok(Self {
            major_version,
            minor_version,
            items,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) -> Result<()> {
        writer.u16(self.major_version);
        writer.u16(self.minor_version);
        writer.u32(self.items.len() as u32);
        for item in &self.items {
            let mut body = Writer::new();
            write_annotation(&mut body, item)?;
            let body = body.into_bytes();
            writer.u32(body.len() as u32 + 4);
            writer.bytes(&body);
        }
        Ok(())
    }
}

fn read_annotation(reader: &mut Reader<'_>) -> Result<Annotation> {
    let kind = reader.key()?;
    let open = reader.bool()?;
    let flags = reader.u8()?;
    let optional_blocks = reader.u16()?;
    let icon = reader.rect()?;
    let popup = reader.rect()?;
    let color = Color::read(reader)?;
    let author = reader.pascal(2)?;
    let name = reader.pascal(2)?;
    let modified = reader.pascal(2)?;
    let _content_length = reader.u32()?;
    let marker = reader.key()?;
    let length = reader.u32()?;
    let data = reader.bytes(length as usize)?;
    let content = if &kind.0 == b"txtA" {
        if data.len() >= 2 && data[..2] == [0xfe, 0xff] {
            let units: Vec<u16> = data[2..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_be_bytes(*pair))
                .collect();
            AnnotationContent::Text(String::from_utf16_lossy(&units))
        } else {
            AnnotationContent::Text(crate::io::mac_roman::decode(data))
        }
    } else {
        AnnotationContent::Sound(data.to_vec())
    };
    Ok(Annotation {
        kind,
        open,
        flags,
        optional_blocks,
        icon,
        popup,
        color,
        author,
        name,
        modified,
        marker,
        content,
    })
}

fn write_annotation(writer: &mut Writer, item: &Annotation) -> Result<()> {
    writer.key(item.kind);
    writer.bool(item.open);
    writer.u8(item.flags);
    writer.u16(item.optional_blocks);
    writer.rect(item.icon);
    writer.rect(item.popup);
    item.color.write(writer);
    writer.pascal(&item.author, 2);
    writer.pascal(&item.name, 2);
    writer.pascal(&item.modified, 2);
    let data = match &item.content {
        AnnotationContent::Text(text) => {
            let mut bytes = vec![0xfe, 0xff];
            for unit in text.encode_utf16() {
                bytes.extend_from_slice(&unit.to_be_bytes());
            }
            bytes
        }
        AnnotationContent::Sound(bytes) => bytes.clone(),
    };
    writer.u32(data.len() as u32 + 12);
    writer.key(item.marker);
    writer.section(Width::Four, Padding::Trailing(4), |writer| {
        writer.bytes(&data);
        Ok(())
    })
}
