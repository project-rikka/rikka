use crate::error::Result;
use crate::io::{Reader, Writer};
use crate::path::PathData;

#[derive(Debug, Clone, PartialEq)]
pub struct VectorMask {
    pub version: u32,
    pub flags: u32,
    pub path: PathData,
    pub stroke_variant: bool,
}

impl VectorMask {
    pub fn new(path: PathData) -> Self {
        Self {
            version: 3,
            flags: 0,
            path,
            stroke_variant: false,
        }
    }

    pub fn inverted(&self) -> bool {
        self.flags & 1 != 0
    }

    pub fn unlinked(&self) -> bool {
        self.flags & 2 != 0
    }

    pub fn disabled(&self) -> bool {
        self.flags & 4 != 0
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.flags = if disabled {
            self.flags | 4
        } else {
            self.flags & !4
        };
    }

    pub(crate) fn read(reader: &mut Reader<'_>, stroke_variant: bool) -> Result<Self> {
        let version = reader.u32()?;
        let flags = reader.u32()?;
        let path = PathData::read(reader)?;
        Ok(Self {
            version,
            flags,
            path,
            stroke_variant,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) {
        writer.u32(self.version);
        writer.u32(self.flags);
        self.path.write(writer);
    }
}
