use crate::color::Color;
use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::io::{Reader, Writer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FilterMask {
    pub color: Color,
    pub opacity: u16,
}

impl FilterMask {
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            color: Color::read(reader)?,
            opacity: reader.u16()?,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) {
        self.color.write(writer);
        writer.u16(self.opacity);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UserMask {
    pub color: Color,
    pub opacity: u16,
    pub flag: u8,
}

impl UserMask {
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let color = Color::read(reader)?;
        let opacity = reader.u16()?;
        let flag = reader.u8()?;
        reader.skip(1.min(reader.remaining()))?;
        Ok(Self {
            color,
            opacity,
            flag,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) {
        self.color.write(writer);
        writer.u16(self.opacity);
        writer.u8(self.flag);
        writer.u8(0);
    }
}

#[derive(Clone, PartialEq)]
pub struct FilterChannel {
    pub compression: u16,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for FilterChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "FilterChannel({}, {} bytes)",
            self.compression,
            self.data.len()
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilterExtra {
    pub rect: Rect,
    pub channel: FilterChannel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilterEffect {
    pub id: String,
    pub version: u32,
    pub rect: Rect,
    pub depth: u32,
    pub channels: Vec<Option<FilterChannel>>,
    pub extra: Option<FilterExtra>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilterEffects {
    pub legacy_key: bool,
    pub version: u32,
    pub effects: Vec<FilterEffect>,
}

fn read_channel(reader: &mut Reader<'_>) -> Result<FilterChannel> {
    let length = reader.u64()?;
    if length < 2 {
        return Err(reader.malformed("filter effect channel"));
    }
    let compression = reader.u16()?;
    let data = reader
        .bytes(usize::try_from(length - 2).map_err(|_| Error::Oversized)?)?
        .to_vec();
    Ok(FilterChannel { compression, data })
}

fn write_channel(writer: &mut Writer, channel: &FilterChannel) {
    writer.u64(channel.data.len() as u64 + 2);
    writer.u16(channel.compression);
    writer.bytes(&channel.data);
}

impl FilterEffects {
    pub(crate) fn read(reader: &mut Reader<'_>, legacy_key: bool) -> Result<Self> {
        let version = reader.u32()?;
        let mut effects = Vec::new();
        while reader.remaining() > 8 {
            let length = reader.u64()?;
            let mut body = reader.sub(length)?;
            let id = String::from_utf8_lossy(body.pascal_bytes(1)?).into_owned();
            let effect_version = body.u32()?;
            let inner_length = body.u64()?;
            let mut inner = body.sub(inner_length)?;
            let rect = inner.rect()?;
            let depth = inner.u32()?;
            let count = inner.u32()?;
            let mut channels = Vec::with_capacity(count.min(64) as usize + 2);
            for _ in 0..count.saturating_add(2) {
                if inner.u32()? == 0 {
                    channels.push(None);
                } else {
                    channels.push(Some(read_channel(&mut inner)?));
                }
            }
            let extra = if body.remaining() > 0 && body.u8()? != 0 {
                let rect = body.rect()?;
                Some(FilterExtra {
                    rect,
                    channel: read_channel(&mut body)?,
                })
            } else {
                None
            };
            effects.push(FilterEffect {
                id,
                version: effect_version,
                rect,
                depth,
                channels,
                extra,
            });
            let padding = (length.div_ceil(4) * 4 - length) as usize;
            reader.skip(padding.min(reader.remaining()))?;
        }
        Ok(Self {
            legacy_key,
            version,
            effects,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) -> Result<()> {
        writer.u32(self.version);
        for effect in &self.effects {
            let mut body = Writer::new();
            body.pascal_bytes(effect.id.as_bytes(), 1);
            body.u32(effect.version);
            let mut inner = Writer::new();
            inner.rect(effect.rect);
            inner.u32(effect.depth);
            inner.u32(effect.channels.len().saturating_sub(2) as u32);
            for channel in &effect.channels {
                match channel {
                    Some(channel) => {
                        inner.u32(1);
                        write_channel(&mut inner, channel);
                    }
                    None => inner.u32(0),
                }
            }
            let inner = inner.into_bytes();
            body.u64(inner.len() as u64);
            body.bytes(&inner);
            if let Some(extra) = &effect.extra {
                body.u8(1);
                body.rect(extra.rect);
                write_channel(&mut body, &extra.channel);
            }
            let body = body.into_bytes();
            writer.u64(body.len() as u64);
            writer.bytes(&body);
            writer.zeros(body.len().div_ceil(4) * 4 - body.len());
        }
        Ok(())
    }
}
