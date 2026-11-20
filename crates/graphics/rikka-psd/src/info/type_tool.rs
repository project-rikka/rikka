use crate::descriptor::{Descriptor, read_versioned, write_versioned};
use crate::error::Result;
use crate::io::{Reader, Writer};

#[derive(Debug, Clone, PartialEq)]
pub struct TypeTool {
    pub version: u16,
    pub transform: [f64; 6],
    pub text_version: u16,
    pub text: Descriptor,
    pub warp_version: u16,
    pub warp: Descriptor,
    pub bounds: [f32; 4],
}

impl TypeTool {
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let version = reader.u16()?;
        let mut transform = [0.0; 6];
        for value in &mut transform {
            *value = reader.f64()?;
        }
        let text_version = reader.u16()?;
        let text = read_versioned(reader)?;
        let warp_version = reader.u16()?;
        let warp = read_versioned(reader)?;
        let mut bounds = [0.0f32; 4];
        for value in &mut bounds {
            *value = f32::from_bits(reader.u32()?);
        }
        Ok(Self {
            version,
            transform,
            text_version,
            text,
            warp_version,
            warp,
            bounds,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) -> Result<()> {
        writer.u16(self.version);
        for value in self.transform {
            writer.f64(value);
        }
        writer.u16(self.text_version);
        write_versioned(writer, &self.text)?;
        writer.u16(self.warp_version);
        write_versioned(writer, &self.warp)?;
        for value in self.bounds {
            writer.u32(value.to_bits());
        }
        Ok(())
    }
}
