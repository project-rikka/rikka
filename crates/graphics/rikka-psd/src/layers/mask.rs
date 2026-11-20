use crate::error::Result;
use crate::geometry::Rect;
use crate::io::{Reader, Writer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MaskFlags(pub u8);

impl MaskFlags {
    pub fn relative_to_layer(self) -> bool {
        self.0 & 0x01 != 0
    }

    pub fn disabled(self) -> bool {
        self.0 & 0x02 != 0
    }

    pub fn inverted(self) -> bool {
        self.0 & 0x04 != 0
    }

    pub fn from_other_data(self) -> bool {
        self.0 & 0x08 != 0
    }

    pub fn has_parameters(self) -> bool {
        self.0 & 0x10 != 0
    }

    pub fn with_disabled(self, disabled: bool) -> Self {
        MaskFlags(if disabled {
            self.0 | 0x02
        } else {
            self.0 & !0x02
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MaskParameters {
    pub user_density: Option<u8>,
    pub user_feather: Option<f64>,
    pub vector_density: Option<u8>,
    pub vector_feather: Option<f64>,
}

impl MaskParameters {
    fn is_empty(&self) -> bool {
        self.user_density.is_none()
            && self.user_feather.is_none()
            && self.vector_density.is_none()
            && self.vector_feather.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RealMask {
    pub flags: MaskFlags,
    pub default_color: u8,
    pub bounds: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerMask {
    pub bounds: Rect,
    pub default_color: u8,
    pub flags: MaskFlags,
    pub real: Option<RealMask>,
    pub parameters: MaskParameters,
}

impl LayerMask {
    pub fn new(bounds: Rect, default_color: u8) -> Self {
        Self {
            bounds,
            default_color,
            flags: MaskFlags::default(),
            real: None,
            parameters: MaskParameters::default(),
        }
    }

    pub(crate) fn read(reader: &mut Reader<'_>, has_real: bool) -> Result<Option<Self>> {
        if reader.is_empty() {
            return Ok(None);
        }
        let length = reader.remaining();
        let bounds = reader.rect()?;
        let default_color = reader.u8()?;
        let flags = MaskFlags(reader.u8()?);
        let real = if has_real && length >= 36 {
            let real_flags = MaskFlags(reader.u8()?);
            let real_color = reader.u8()?;
            Some(RealMask {
                flags: real_flags,
                default_color: real_color,
                bounds: reader.rect()?,
            })
        } else {
            None
        };
        let mut parameters = MaskParameters::default();
        if flags.has_parameters() && reader.remaining() >= 1 {
            let present = reader.u8()?;
            if present & 1 != 0 {
                parameters.user_density = Some(reader.u8()?);
            }
            if present & 2 != 0 {
                parameters.user_feather = Some(reader.f64()?);
            }
            if present & 4 != 0 {
                parameters.vector_density = Some(reader.u8()?);
            }
            if present & 8 != 0 {
                parameters.vector_feather = Some(reader.f64()?);
            }
        }
        Ok(Some(Self {
            bounds,
            default_color,
            flags: MaskFlags(flags.0 & !0x10),
            real,
            parameters,
        }))
    }

    pub(crate) fn write(&self, writer: &mut Writer) {
        let start = writer.len();
        writer.rect(self.bounds);
        writer.u8(self.default_color);
        let flags = if self.parameters.is_empty() {
            self.flags.0 & !0x10
        } else {
            self.flags.0 | 0x10
        };
        writer.u8(flags);
        if let Some(real) = &self.real {
            writer.u8(real.flags.0);
            writer.u8(real.default_color);
            writer.rect(real.bounds);
        }
        if !self.parameters.is_empty() {
            let parameters = &self.parameters;
            let present = u8::from(parameters.user_density.is_some())
                | u8::from(parameters.user_feather.is_some()) << 1
                | u8::from(parameters.vector_density.is_some()) << 2
                | u8::from(parameters.vector_feather.is_some()) << 3;
            writer.u8(present);
            if let Some(value) = parameters.user_density {
                writer.u8(value);
            }
            if let Some(value) = parameters.user_feather {
                writer.f64(value);
            }
            if let Some(value) = parameters.vector_density {
                writer.u8(value);
            }
            if let Some(value) = parameters.vector_feather {
                writer.f64(value);
            }
        }
        if writer.len() - start < 20 {
            writer.zeros(20 - (writer.len() - start));
        }
        writer.align_from(start, 4);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendRange {
    pub source: [u8; 4],
    pub destination: [u8; 4],
}

impl BlendRange {
    pub const FULL: BlendRange = BlendRange {
        source: [0, 0, 255, 255],
        destination: [0, 0, 255, 255],
    };

    pub fn is_full(&self) -> bool {
        *self == BlendRange::FULL
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlendingRanges {
    pub gray: Option<BlendRange>,
    pub channels: Vec<BlendRange>,
}

impl BlendingRanges {
    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Self> {
        let mut ranges = Vec::new();
        while reader.remaining() >= 8 {
            let mut source = [0u8; 4];
            let mut destination = [0u8; 4];
            source.copy_from_slice(reader.bytes(4)?);
            destination.copy_from_slice(reader.bytes(4)?);
            ranges.push(BlendRange {
                source,
                destination,
            });
        }
        let mut ranges = ranges.into_iter();
        Ok(Self {
            gray: ranges.next(),
            channels: ranges.collect(),
        })
    }

    pub(crate) fn write(&self, writer: &mut Writer) {
        if let Some(gray) = &self.gray {
            for range in std::iter::once(gray).chain(&self.channels) {
                writer.bytes(&range.source);
                writer.bytes(&range.destination);
            }
        }
    }

    pub fn is_neutral(&self) -> bool {
        self.gray.is_none_or(|gray| gray.is_full()) && self.channels.iter().all(BlendRange::is_full)
    }
}
