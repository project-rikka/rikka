use crate::blend::BlendMode;
use crate::compression::{Compression, Encoded};
use crate::error::{Error, Result};
use crate::info;
use crate::io::{Padding, Reader, Width, Writer};
use crate::layers::channel::{Channel, ChannelId};
use crate::layers::layer::{Layer, LayerFlags};
use crate::layers::mask::{BlendingRanges, LayerMask};

pub(crate) struct LayerInfo {
    pub(crate) layers: Vec<Layer>,
    pub(crate) merged_alpha: bool,
}

pub(crate) fn read_layer_info(reader: &mut Reader<'_>, psb: bool) -> Result<LayerInfo> {
    if reader.remaining() < 2 {
        return Ok(LayerInfo {
            layers: Vec::new(),
            merged_alpha: false,
        });
    }
    let count = reader.i16()?;
    let merged_alpha = count < 0;
    let count = usize::from(count.unsigned_abs());
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        records.push(read_record(reader, psb)?);
    }
    let mut layers = Vec::with_capacity(count);
    for (mut layer, lengths) in records {
        for (id, length) in lengths {
            let mut body = reader.sub(length.min(reader.remaining() as u64))?;
            let data = if body.remaining() < 2 {
                Encoded::Raw(Vec::new())
            } else {
                let code = body.u16()?;
                let compression = Compression::from_code(code).ok_or(Error::Invalid {
                    what: "channel compression",
                    value: i64::from(code),
                })?;
                let height = layer.channel_bounds(id).height();
                Encoded::read_channel(&mut body, compression, height, psb)?
            };
            layer.channels.push(Channel { id, data });
        }
        layers.push(layer);
    }
    Ok(LayerInfo {
        layers,
        merged_alpha,
    })
}

fn read_record(reader: &mut Reader<'_>, psb: bool) -> Result<(Layer, Vec<(ChannelId, u64)>)> {
    let bounds = reader.rect()?;
    let channel_count = reader.u16()?;
    if channel_count > 64 {
        return Err(reader.malformed("layer channel count"));
    }
    let mut lengths = Vec::with_capacity(usize::from(channel_count));
    for _ in 0..channel_count {
        let id = ChannelId::from_code(reader.i16()?);
        lengths.push((id, reader.length(psb)?));
    }
    let signature = reader.key()?;
    if !matches!(&signature.0, b"8BIM" | b"8B64") {
        return Err(reader.malformed("layer blend signature"));
    }
    let blend_mode = BlendMode::from_key(reader.key()?);
    let opacity = reader.u8()?;
    let clipping = reader.u8()? != 0;
    let flags = LayerFlags(reader.u8()?);
    reader.skip(1)?;
    let extra_length = u64::from(reader.u32()?);
    let mut extra = reader.sub(extra_length)?;
    let has_real = lengths.iter().any(|(id, _)| *id == ChannelId::RealUserMask);
    let mask_length = u64::from(extra.u32()?);
    let mut mask_reader = extra.sub(mask_length)?;
    let mask = LayerMask::read(&mut mask_reader, has_real)?;
    let ranges_length = u64::from(extra.u32()?);
    let mut ranges_reader = extra.sub(ranges_length)?;
    let blending_ranges = BlendingRanges::read(&mut ranges_reader)?;
    let legacy_name = extra.pascal(1)?;
    extra.skip_zero_padding(3);
    let info = info::read_blocks(&mut extra, psb)?;
    Ok((
        Layer {
            legacy_name,
            bounds,
            channels: Vec::with_capacity(lengths.len()),
            blend_mode,
            opacity,
            clipping,
            flags,
            mask,
            blending_ranges,
            info,
        },
        lengths,
    ))
}

pub(crate) fn write_layer_info(
    writer: &mut Writer,
    layers: &[&Layer],
    merged_alpha: bool,
    psb: bool,
) -> Result<()> {
    if layers.is_empty() {
        return Ok(());
    }
    let count = i16::try_from(layers.len()).map_err(|_| Error::Oversized)?;
    writer.i16(if merged_alpha { -count } else { count });
    for layer in layers {
        write_record(writer, layer, psb)?;
    }
    for layer in layers {
        for channel in &layer.channels {
            let height = layer.channel_bounds(channel.id).height();
            if let Encoded::Rle { rows, .. } = &channel.data
                && rows.len() != height as usize
            {
                return Err(Error::Planes("an RLE channel does not match its bounds"));
            }
            writer.u16(channel.data.compression().code());
            channel.data.write_body(writer, psb)?;
        }
    }
    Ok(())
}

fn write_record(writer: &mut Writer, layer: &Layer, psb: bool) -> Result<()> {
    writer.rect(layer.bounds);
    writer.u16(layer.channels.len() as u16);
    for channel in &layer.channels {
        writer.i16(channel.id.code());
        writer.length(channel.data.file_len(psb), Width::from_wide(psb))?;
    }
    writer.code(b"8BIM");
    writer.key(layer.blend_mode.key());
    writer.u8(layer.opacity);
    writer.u8(u8::from(layer.clipping));
    writer.u8(layer.flags.0);
    writer.u8(0);
    writer.section(Width::Four, Padding::Counted(1), |writer| {
        writer.section(Width::Four, Padding::Counted(1), |writer| {
            if let Some(mask) = &layer.mask {
                mask.write(writer);
            }
            Ok(())
        })?;
        writer.section(Width::Four, Padding::Counted(1), |writer| {
            layer.blending_ranges.write(writer);
            Ok(())
        })?;
        writer.pascal(&layer.legacy_name, 4);
        for item in &layer.info {
            info::write_block(writer, item, psb)?;
        }
        Ok(())
    })
}
