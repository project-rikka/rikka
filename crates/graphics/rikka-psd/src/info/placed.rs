use crate::descriptor::{Descriptor, read_versioned, write_versioned};
use crate::error::Result;
use crate::io::{Reader, Writer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacedKind {
    Unknown,
    Vector,
    Raster,
    ImageStack,
    Other(i32),
}

impl PlacedKind {
    fn from_code(code: i32) -> Self {
        match code {
            0 => PlacedKind::Unknown,
            1 => PlacedKind::Vector,
            2 => PlacedKind::Raster,
            3 => PlacedKind::ImageStack,
            other => PlacedKind::Other(other),
        }
    }

    fn code(self) -> i32 {
        match self {
            PlacedKind::Unknown => 0,
            PlacedKind::Vector => 1,
            PlacedKind::Raster => 2,
            PlacedKind::ImageStack => 3,
            PlacedKind::Other(code) => code,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlacedLayer {
    pub version: i32,
    pub unique_id: String,
    pub page: i32,
    pub total_pages: i32,
    pub anti_alias: i32,
    pub kind: PlacedKind,
    pub transform: [f64; 8],
    pub warp_version: i32,
    pub warp: Descriptor,
    pub legacy_key: bool,
}

impl PlacedLayer {
    pub(crate) fn read(reader: &mut Reader<'_>, legacy_key: bool) -> Result<Self> {
        reader.expect(b"plcL")?;
        let version = reader.i32()?;
        let unique_id = String::from_utf8_lossy(reader.pascal_bytes(1)?).into_owned();
        let page = reader.i32()?;
        let total_pages = reader.i32()?;
        let anti_alias = reader.i32()?;
        let kind = PlacedKind::from_code(reader.i32()?);
        let mut transform = [0.0; 8];
        for value in &mut transform {
            *value = reader.f64()?;
        }
        let warp_version = reader.i32()?;
        let warp = read_versioned(reader)?;
        Ok(Self {
            version,
            unique_id,
            page,
            total_pages,
            anti_alias,
            kind,
            transform,
            warp_version,
            warp,
            legacy_key,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) -> Result<()> {
        writer.code(b"plcL");
        writer.i32(self.version);
        writer.pascal_bytes(self.unique_id.as_bytes(), 1);
        writer.i32(self.page);
        writer.i32(self.total_pages);
        writer.i32(self.anti_alias);
        writer.i32(self.kind.code());
        for value in self.transform {
            writer.f64(value);
        }
        writer.i32(self.warp_version);
        write_versioned(writer, &self.warp)
    }
}
