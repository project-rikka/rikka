use crate::compression::Encoded;
use crate::document::ColorMode;
use crate::error::{Error, Result};
use crate::geometry::Rect;
use crate::io::{Padding, Reader, Width, Writer};
use crate::pixels::{Depth, Plane};

#[derive(Clone, PartialEq)]
pub struct VirtualChannel {
    pub depth: u32,
    pub rect: Rect,
    pub pixel_depth: u16,
    pub compression: u8,
    pub data: Vec<u8>,
}

impl std::fmt::Debug for VirtualChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VirtualChannel")
            .field("rect", &self.rect)
            .field("pixel_depth", &self.pixel_depth)
            .field("compression", &self.compression)
            .field("bytes", &self.data.len())
            .finish()
    }
}

impl VirtualChannel {
    pub fn from_plane(plane: &Plane) -> Self {
        let encoded = Encoded::encode(plane, crate::compression::Compression::Rle);
        let mut data = Writer::new();
        encoded
            .write_body(&mut data, false)
            .expect("pattern rows are narrower than 65536 bytes");
        Self {
            depth: u32::from(plane.depth().bits()),
            rect: Rect::from_size(0, 0, plane.width(), plane.height()),
            pixel_depth: plane.depth().bits(),
            compression: 1,
            data: data.into_bytes(),
        }
    }

    pub fn plane(&self) -> Result<Plane> {
        let width = self.rect.width();
        let height = self.rect.height();
        let depth = Depth::from_bits(self.pixel_depth).ok_or(Error::Invalid {
            what: "pattern depth",
            value: i64::from(self.pixel_depth),
        })?;
        let encoded = match self.compression {
            0 => Encoded::Raw(self.data.clone()),
            1 => {
                let mut reader = Reader::new(&self.data);
                Encoded::read_channel(
                    &mut reader,
                    crate::compression::Compression::Rle,
                    height,
                    false,
                )?
            }
            2 => Encoded::Zip(self.data.clone()),
            3 => Encoded::ZipPredicted(self.data.clone()),
            other => {
                return Err(Error::Invalid {
                    what: "pattern compression",
                    value: i64::from(other),
                });
            }
        };
        encoded.decode(width, height, depth)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VirtualSlot {
    Absent,
    Empty,
    Channel(VirtualChannel),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VirtualArray {
    pub version: u32,
    pub rect: Rect,
    pub slots: Vec<VirtualSlot>,
}

impl VirtualArray {
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let version = reader.u32()?;
        let length = u64::from(reader.u32()?);
        let mut body = reader.sub(length)?;
        let rect = body.rect()?;
        let count = body.u32()?;
        let mut slots = Vec::with_capacity(count.min(64) as usize + 2);
        for _ in 0..count.saturating_add(2) {
            if body.is_empty() {
                break;
            }
            if body.u32()? == 0 {
                slots.push(VirtualSlot::Absent);
                continue;
            }
            let length = body.u32()?;
            if length == 0 {
                slots.push(VirtualSlot::Empty);
                continue;
            }
            let mut channel = body.sub(u64::from(length))?;
            let depth = channel.u32()?;
            let rect = channel.rect()?;
            let pixel_depth = channel.u16()?;
            let compression = channel.u8()?;
            slots.push(VirtualSlot::Channel(VirtualChannel {
                depth,
                rect,
                pixel_depth,
                compression,
                data: channel.take_rest().to_vec(),
            }));
        }
        Ok(Self {
            version,
            rect,
            slots,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) -> Result<()> {
        writer.u32(self.version);
        writer.section(Width::Four, Padding::Counted(1), |writer| {
            writer.rect(self.rect);
            writer.u32(self.slots.len().saturating_sub(2) as u32);
            for slot in &self.slots {
                match slot {
                    VirtualSlot::Absent => writer.u32(0),
                    VirtualSlot::Empty => {
                        writer.u32(1);
                        writer.u32(0);
                    }
                    VirtualSlot::Channel(channel) => {
                        writer.u32(1);
                        writer.section(Width::Four, Padding::Counted(1), |writer| {
                            writer.u32(channel.depth);
                            writer.rect(channel.rect);
                            writer.u16(channel.pixel_depth);
                            writer.u8(channel.compression);
                            writer.bytes(&channel.data);
                            Ok(())
                        })?;
                    }
                }
            }
            Ok(())
        })
    }

    pub fn channels(&self) -> impl Iterator<Item = &VirtualChannel> {
        self.slots.iter().filter_map(|slot| match slot {
            VirtualSlot::Channel(channel) => Some(channel),
            _ => None,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pattern {
    pub version: u32,
    pub mode: ColorMode,
    pub point: (i16, i16),
    pub name: String,
    pub id: String,
    pub palette: Option<Vec<[u8; 3]>>,
    pub image: VirtualArray,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PatternPixels {
    pub width: u32,
    pub height: u32,
    pub mode: ColorMode,
    pub color: Vec<Plane>,
    pub alpha: Option<Plane>,
}

impl Pattern {
    pub fn new(
        name: impl Into<String>,
        id: impl Into<String>,
        mode: ColorMode,
        color: &[Plane],
        alpha: Option<&Plane>,
    ) -> Result<Self> {
        let first = color
            .first()
            .ok_or(Error::Planes("a pattern needs a colour plane"))?;
        let (width, height) = (first.width(), first.height());
        if color
            .iter()
            .chain(alpha)
            .any(|plane| plane.width() != width || plane.height() != height)
        {
            return Err(Error::Planes("pattern planes differ in size"));
        }
        let mut slots: Vec<VirtualSlot> = color
            .iter()
            .map(|plane| VirtualSlot::Channel(VirtualChannel::from_plane(plane)))
            .collect();
        slots.push(alpha.map_or(VirtualSlot::Absent, |plane| {
            VirtualSlot::Channel(VirtualChannel::from_plane(plane))
        }));
        slots.push(VirtualSlot::Absent);
        Ok(Self {
            version: 1,
            mode,
            point: (0, 0),
            name: name.into(),
            id: id.into(),
            palette: None,
            image: VirtualArray {
                version: 3,
                rect: Rect::from_size(0, 0, width, height),
                slots,
            },
        })
    }

    pub fn width(&self) -> u32 {
        self.image.rect.width()
    }

    pub fn height(&self) -> u32 {
        self.image.rect.height()
    }

    pub fn pixels(&self) -> Result<PatternPixels> {
        let color_count = match self.mode {
            ColorMode::Rgb | ColorMode::Lab => 3,
            ColorMode::Cmyk => 4,
            _ => 1,
        };
        let (width, height) = (self.width(), self.height());
        if width == 0 || height == 0 {
            return Err(Error::Planes("the pattern is empty"));
        }
        let mut planes = self
            .image
            .channels()
            .take(color_count + 1)
            .map(VirtualChannel::plane)
            .collect::<Result<Vec<_>>>()?;
        if planes.len() < color_count {
            return Err(Error::Planes("the pattern lacks colour channels"));
        }
        if planes
            .iter()
            .any(|plane| plane.width() != width || plane.height() != height)
        {
            return Err(Error::Planes("pattern planes differ in size"));
        }
        let alpha = (planes.len() > color_count).then(|| planes.remove(color_count));
        Ok(PatternPixels {
            width,
            height,
            mode: self.mode,
            color: planes,
            alpha,
        })
    }

    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let version = reader.u32()?;
        let mode_code = reader.u32()?;
        let mode = ColorMode::from_code(u16::try_from(mode_code).unwrap_or(u16::MAX)).ok_or(
            Error::Invalid {
                what: "pattern colour mode",
                value: i64::from(mode_code),
            },
        )?;
        let point = (reader.i16()?, reader.i16()?);
        let name = reader.unicode()?;
        let id = String::from_utf8_lossy(reader.pascal_bytes(1)?).into_owned();
        let palette = if mode == ColorMode::Indexed {
            let mut entries = Vec::with_capacity(256);
            for _ in 0..256 {
                let bytes = reader.bytes(3)?;
                entries.push([bytes[0], bytes[1], bytes[2]]);
            }
            reader.skip(4)?;
            Some(entries)
        } else {
            None
        };
        let image = VirtualArray::read(reader)?;
        Ok(Self {
            version,
            mode,
            point,
            name,
            id,
            palette,
            image,
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) -> Result<()> {
        writer.u32(self.version);
        writer.u32(u32::from(self.mode.code()));
        writer.i16(self.point.0);
        writer.i16(self.point.1);
        writer.unicode_terminated(&self.name);
        writer.pascal_bytes(self.id.as_bytes(), 1);
        if let Some(palette) = &self.palette {
            for index in 0..256 {
                writer.bytes(palette.get(index).unwrap_or(&[0; 3]));
            }
            writer.zeros(4);
        }
        self.image.write(writer)
    }

    pub(crate) fn read_list(reader: &mut Reader<'_>) -> Result<Vec<Pattern>> {
        let mut patterns = Vec::new();
        while reader.remaining() >= 4 {
            let length = u64::from(reader.u32()?);
            if length == 0 {
                break;
            }
            let mut body = reader.sub(length)?;
            patterns.push(Pattern::read(&mut body)?);
            let padding = (length.div_ceil(4) * 4 - length) as usize;
            reader.skip(padding.min(reader.remaining()))?;
        }
        Ok(patterns)
    }

    pub(crate) fn write_list(writer: &mut Writer, patterns: &[Pattern]) -> Result<()> {
        for pattern in patterns {
            writer.section(Width::Four, Padding::Trailing(4), |writer| {
                pattern.write(writer)
            })?;
        }
        Ok(())
    }

    pub fn read_pat(bytes: &[u8]) -> Result<Vec<Pattern>> {
        let mut reader = Reader::new(bytes);
        reader.expect(b"8BPT")?;
        let _version = reader.u16()?;
        let count = reader.u32()?;
        let mut patterns = Vec::with_capacity(count.min(4096) as usize);
        for _ in 0..count {
            patterns.push(Pattern::read(&mut reader)?);
        }
        Ok(patterns)
    }

    pub fn write_pat(patterns: &[Pattern]) -> Result<Vec<u8>> {
        let mut writer = Writer::new();
        writer.code(b"8BPT");
        writer.u16(1);
        writer.u32(patterns.len() as u32);
        for pattern in patterns {
            pattern.write(&mut writer)?;
        }
        Ok(writer.into_bytes())
    }
}
