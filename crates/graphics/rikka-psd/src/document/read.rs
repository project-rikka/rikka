use super::header::{ColorMode, ColorModeData, FileHeader, GlobalMask};
use super::{Composite, Document};
use crate::compression::Encoded;
use crate::error::Result;
use crate::info;
use crate::io::Reader;
use crate::layers::record::{LayerInfo, read_layer_info};
use crate::layers::tree;
use crate::resources::Resources;

impl Document {
    pub fn read(bytes: &[u8]) -> Result<Document> {
        let FileHeader {
            version,
            channels,
            width,
            height,
            depth,
            color_mode,
        } = FileHeader::read(bytes)?;
        let psb = version.is_large();
        let mut reader = Reader::new(bytes);
        reader.skip(FileHeader::LEN)?;

        let color_length = u64::from(reader.u32()?);
        let mut color_reader = reader.sub(color_length)?;
        let color_mode_data = if color_length == 0 {
            ColorModeData::Empty
        } else if color_mode == ColorMode::Indexed && color_length >= 768 {
            let data = color_reader.bytes(768)?;
            ColorModeData::Palette(
                (0..256)
                    .map(|index| [data[index], data[256 + index], data[512 + index]])
                    .collect(),
            )
        } else {
            ColorModeData::Raw(color_reader.take_rest().to_vec())
        };

        let resources_length = u64::from(reader.u32()?);
        let mut resources_reader = reader.sub(resources_length)?;
        let resources = Resources::read(&mut resources_reader)?;

        let section_length = reader.length(psb)?;
        let mut section = reader.sub(section_length.min(reader.remaining() as u64))?;
        let mut layer_info = LayerInfo {
            layers: Vec::new(),
            merged_alpha: false,
        };
        let mut global_mask = None;
        let mut document_info = Vec::new();
        if !section.is_empty() {
            let info_length = section.length(psb)?;
            let mut info_reader = section.sub(info_length.min(section.remaining() as u64))?;
            layer_info = read_layer_info(&mut info_reader, psb)?;
            if section.remaining() >= 4 {
                let mask_length = u64::from(section.u32()?);
                let mut mask_reader = section.sub(mask_length.min(section.remaining() as u64))?;
                if mask_reader.remaining() >= 13 {
                    let overlay_color_space = mask_reader.u16()?;
                    let mut color = [0u16; 4];
                    for value in &mut color {
                        *value = mask_reader.u16()?;
                    }
                    global_mask = Some(GlobalMask {
                        overlay_color_space,
                        color,
                        opacity: mask_reader.u16()?,
                        kind: mask_reader.u8()?,
                    });
                }
            }
            for block in info::read_raw_blocks(&mut section, psb)? {
                if matches!(&block.key.0, b"Lr16" | b"Lr32" | b"Layr")
                    && layer_info.layers.is_empty()
                {
                    let mut body = block.body;
                    layer_info = read_layer_info(&mut body, psb)?;
                    continue;
                }
                document_info.push(info::decode_block(block));
            }
        }

        let planes = Encoded::read_composite(
            &mut reader,
            usize::from(channels),
            width,
            height,
            depth,
            psb,
        )?;
        Ok(Document {
            version,
            width,
            height,
            depth,
            color_mode,
            color_mode_data,
            resources,
            layers: tree::build(layer_info.layers),
            merged_alpha: layer_info.merged_alpha,
            global_mask,
            info: document_info,
            composite: Composite { planes },
        })
    }
}
