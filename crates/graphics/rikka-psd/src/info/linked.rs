use crate::descriptor::{Descriptor, read_versioned, write_versioned};
use crate::error::{Error, Result};
use crate::io::{Reader, Writer};
use crate::key::Key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Data,
    External,
    Alias,
}

impl LinkKind {
    fn from_key(key: Key) -> Option<Self> {
        match &key.0 {
            b"liFD" => Some(LinkKind::Data),
            b"liFE" => Some(LinkKind::External),
            b"liFA" => Some(LinkKind::Alias),
            _ => None,
        }
    }

    fn key(self) -> &'static [u8; 4] {
        match self {
            LinkKind::Data => b"liFD",
            LinkKind::External => b"liFE",
            LinkKind::Alias => b"liFA",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinkTime {
    pub year: u32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub seconds: f64,
}

#[derive(Clone, PartialEq)]
pub struct LinkedFile {
    pub kind: LinkKind,
    pub version: u32,
    pub unique_id: String,
    pub name: String,
    pub file_type: Key,
    pub creator: Key,
    pub open_descriptor: Option<Descriptor>,
    pub link_descriptor: Option<Descriptor>,
    pub time: Option<LinkTime>,
    pub external_size: u64,
    pub data: Vec<u8>,
    pub child_document_id: Option<String>,
    pub asset_modified: Option<f64>,
    pub asset_locked: Option<u8>,
}

impl std::fmt::Debug for LinkedFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkedFile")
            .field("kind", &self.kind)
            .field("unique_id", &self.unique_id)
            .field("name", &self.name)
            .field("file_type", &self.file_type)
            .field("bytes", &self.data.len())
            .finish_non_exhaustive()
    }
}

impl LinkedFile {
    pub fn embedded(
        unique_id: impl Into<String>,
        name: impl Into<String>,
        file_type: Key,
        data: Vec<u8>,
    ) -> Self {
        Self {
            kind: LinkKind::Data,
            version: 7,
            unique_id: unique_id.into(),
            name: name.into(),
            file_type,
            creator: Key([0; 4]),
            open_descriptor: None,
            link_descriptor: None,
            time: None,
            external_size: 0,
            data,
            child_document_id: Some(String::new()),
            asset_modified: Some(0.0),
            asset_locked: Some(0),
        }
    }

    fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let tag = reader.key()?;
        let kind = LinkKind::from_key(tag).ok_or_else(|| reader.malformed("linked file kind"))?;
        let version = reader.u32()?;
        if !(1..=8).contains(&version) {
            return Err(Error::Invalid {
                what: "linked file version",
                value: i64::from(version),
            });
        }
        let unique_id = String::from_utf8_lossy(reader.pascal_bytes(1)?).into_owned();
        let name = reader.unicode()?;
        let file_type = reader.key()?;
        let creator = reader.key()?;
        let data_size = usize::try_from(reader.u64()?).map_err(|_| Error::Oversized)?;
        let open_descriptor = if reader.bool()? {
            Some(read_versioned(reader)?)
        } else {
            None
        };
        let mut link_descriptor = None;
        let mut time = None;
        let mut external_size = 0;
        let mut data = Vec::new();
        match kind {
            LinkKind::External => {
                link_descriptor = Some(read_versioned(reader)?);
                if version > 3 {
                    time = Some(LinkTime {
                        year: reader.u32()?,
                        month: reader.u8()?,
                        day: reader.u8()?,
                        hour: reader.u8()?,
                        minute: reader.u8()?,
                        seconds: reader.f64()?,
                    });
                }
                external_size = reader.u64()?;
                if version > 2 {
                    data = reader.bytes(data_size)?.to_vec();
                }
            }
            LinkKind::Alias => reader.skip(8)?,
            LinkKind::Data => data = reader.bytes(data_size)?.to_vec(),
        }
        let child_document_id = if version >= 5 {
            Some(reader.unicode()?)
        } else {
            None
        };
        let asset_modified = if version >= 6 {
            Some(reader.f64()?)
        } else {
            None
        };
        let asset_locked = if version >= 7 {
            Some(reader.u8()?)
        } else {
            None
        };
        if kind == LinkKind::External && version == 2 {
            data = reader.bytes(data_size)?.to_vec();
        }
        Ok(Self {
            kind,
            version,
            unique_id,
            name,
            file_type,
            creator,
            open_descriptor,
            link_descriptor,
            time,
            external_size,
            data,
            child_document_id,
            asset_modified,
            asset_locked,
        })
    }

    fn write(&self, writer: &mut Writer) -> Result<()> {
        writer.code(self.kind.key());
        writer.u32(self.version);
        writer.pascal_bytes(self.unique_id.as_bytes(), 1);
        writer.unicode_terminated(&self.name);
        writer.key(self.file_type);
        writer.key(self.creator);
        writer.u64(self.data.len() as u64);
        writer.bool(self.open_descriptor.is_some());
        if let Some(descriptor) = &self.open_descriptor {
            write_versioned(writer, descriptor)?;
        }
        match self.kind {
            LinkKind::External => {
                write_versioned(
                    writer,
                    self.link_descriptor
                        .as_ref()
                        .unwrap_or(&Descriptor::new("ExternalFileLink")),
                )?;
                if self.version > 3 {
                    let time = self.time.unwrap_or(LinkTime {
                        year: 1970,
                        month: 1,
                        day: 1,
                        hour: 0,
                        minute: 0,
                        seconds: 0.0,
                    });
                    writer.u32(time.year);
                    writer.u8(time.month);
                    writer.u8(time.day);
                    writer.u8(time.hour);
                    writer.u8(time.minute);
                    writer.f64(time.seconds);
                }
                writer.u64(self.external_size);
                if self.version > 2 {
                    writer.bytes(&self.data);
                }
            }
            LinkKind::Alias => writer.zeros(8),
            LinkKind::Data => writer.bytes(&self.data),
        }
        if self.version >= 5 {
            writer.unicode_terminated(self.child_document_id.as_deref().unwrap_or(""));
        }
        if self.version >= 6 {
            writer.f64(self.asset_modified.unwrap_or(0.0));
        }
        if self.version >= 7 {
            writer.u8(self.asset_locked.unwrap_or(0));
        }
        if self.kind == LinkKind::External && self.version == 2 {
            writer.bytes(&self.data);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkedKey {
    Lnk2,
    LnkD,
    Lnk3,
    LnkE,
}

impl LinkedKey {
    pub(crate) fn from_key(key: Key) -> Option<Self> {
        match &key.0 {
            b"lnk2" => Some(LinkedKey::Lnk2),
            b"lnkD" => Some(LinkedKey::LnkD),
            b"lnk3" => Some(LinkedKey::Lnk3),
            b"lnkE" => Some(LinkedKey::LnkE),
            _ => None,
        }
    }

    pub(crate) fn key(self) -> Key {
        Key(*match self {
            LinkedKey::Lnk2 => b"lnk2",
            LinkedKey::LnkD => b"lnkD",
            LinkedKey::Lnk3 => b"lnk3",
            LinkedKey::LnkE => b"lnkE",
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinkedFiles {
    pub key: LinkedKey,
    pub files: Vec<LinkedFile>,
}

impl LinkedFiles {
    pub(crate) fn read(reader: &mut Reader<'_>, key: LinkedKey) -> Result<Self> {
        let mut files = Vec::new();
        while reader.remaining() > 8 {
            let length = reader.u64()?;
            let mut body = reader.sub(length)?;
            files.push(LinkedFile::read(&mut body)?);
            let padding = (length.div_ceil(4) * 4 - length) as usize;
            reader.skip(padding.min(reader.remaining()))?;
        }
        Ok(Self { key, files })
    }

    pub(crate) fn write(&self, writer: &mut Writer) -> Result<()> {
        for file in &self.files {
            let mut body = Writer::new();
            file.write(&mut body)?;
            let body = body.into_bytes();
            writer.u64(body.len() as u64);
            writer.bytes(&body);
            writer.zeros(body.len().div_ceil(4) * 4 - body.len());
        }
        Ok(())
    }
}
