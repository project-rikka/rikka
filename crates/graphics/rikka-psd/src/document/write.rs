use std::borrow::Cow;

use super::Document;
use super::header::{ColorMode, ColorModeData};
use crate::compression::{Compression, Encoded};
use crate::error::{Error, Result};
use crate::info;
use crate::io::{Padding, Width, Writer};
use crate::key::Key;
use crate::layers::Layer;
use crate::layers::record::write_layer_info;
use crate::layers::tree;
use crate::pixels::{Depth, Plane};

impl Document {
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let psb = self.version.is_large();
        let planes = self.composite_for_writing()?;
        let mut writer = Writer::with_capacity(1 << 16);

        writer.code(b"8BPS");
        writer.u16(self.version.code());
        writer.zeros(6);
        writer.u16(planes.len() as u16);
        writer.u32(self.height);
        writer.u32(self.width);
        writer.u16(self.depth.bits());
        writer.u16(self.color_mode.code());

        writer.section(Width::Four, Padding::Counted(1), |writer| {
            match &self.color_mode_data {
                ColorModeData::Empty => {}
                ColorModeData::Palette(palette) => {
                    for component in 0..3 {
                        for index in 0..256 {
                            writer.u8(palette.get(index).map_or(0, |entry| entry[component]));
                        }
                    }
                }
                ColorModeData::Raw(bytes) => writer.bytes(bytes),
            }
            Ok(())
        })?;

        writer.section(Width::Four, Padding::Counted(1), |writer| {
            self.resources.write(writer)
        })?;

        let placeholder = Layer::default();
        let records = {
            let mut records = Vec::new();
            tree::flatten(&self.layers, &mut records);
            if records.is_empty() && self.merged_alpha {
                records.push(&placeholder);
            }
            records
        };
        let deep = matches!(self.depth, Depth::Sixteen | Depth::ThirtyTwo);
        writer.section(Width::from_wide(psb), Padding::Counted(2), |writer| {
            writer.section(Width::from_wide(psb), Padding::Counted(4), |writer| {
                if !deep {
                    write_layer_info(writer, &records, self.merged_alpha, psb)?;
                }
                Ok(())
            })?;
            writer.section(Width::Four, Padding::Counted(1), |writer| {
                if let Some(mask) = &self.global_mask {
                    writer.u16(mask.overlay_color_space);
                    mask.color.iter().for_each(|&value| writer.u16(value));
                    writer.u16(mask.opacity);
                    writer.u8(mask.kind);
                    writer.zeros(3);
                }
                Ok(())
            })?;
            if deep && !records.is_empty() {
                let key = Key(if self.depth == Depth::Sixteen {
                    *b"Lr16"
                } else {
                    *b"Lr32"
                });
                info::write_keyed(writer, key, psb, |writer| {
                    write_layer_info(writer, &records, self.merged_alpha, psb)
                })?;
            }
            for item in &self.info {
                info::write_block(writer, item, psb)?;
            }
            Ok(())
        })?;

        Encoded::write_composite(&mut writer, &planes, psb)?;
        Ok(writer.into_bytes())
    }

    fn validate(&self) -> Result<()> {
        self.version.check_dimensions(self.width, self.height)?;
        let depth_allowed = match self.color_mode {
            ColorMode::Bitmap => self.depth == Depth::One,
            ColorMode::Indexed | ColorMode::Duotone => self.depth == Depth::Eight,
            ColorMode::Multichannel | ColorMode::Lab | ColorMode::Cmyk => {
                matches!(self.depth, Depth::Eight | Depth::Sixteen)
            }
            ColorMode::Grayscale | ColorMode::Rgb => self.depth != Depth::One,
        };
        if !depth_allowed {
            return Err(Error::ColorMode(self.color_mode.name()));
        }
        if !self.layers.is_empty() && !self.color_mode.supports_layers() {
            return Err(Error::ColorMode(self.color_mode.name()));
        }
        if self.color_mode == ColorMode::Indexed && self.color_mode_data.palette().is_none() {
            return Err(Error::Planes("an indexed document needs a palette"));
        }
        Ok(())
    }

    fn composite_for_writing(&self) -> Result<Cow<'_, [Encoded]>> {
        let planes = &self.composite.planes;
        if planes.is_empty() {
            let count = self.color_mode.color_channels() + usize::from(self.merged_alpha);
            let blank = Plane::filled(self.width, self.height, self.depth, 1.0);
            return Ok(Cow::Owned(vec![
                Encoded::encode(&blank, Compression::Rle);
                count
            ]));
        }
        let first = planes[0].compression();
        let uniform = matches!(first, Compression::Raw | Compression::Rle)
            && planes.iter().all(|plane| plane.compression() == first);
        if uniform {
            return Ok(Cow::Borrowed(planes));
        }
        planes
            .iter()
            .map(|plane| {
                let decoded = plane.decode(self.width, self.height, self.depth)?;
                Ok(Encoded::encode(&decoded, Compression::Rle))
            })
            .collect::<Result<Vec<_>>>()
            .map(Cow::Owned)
    }
}
