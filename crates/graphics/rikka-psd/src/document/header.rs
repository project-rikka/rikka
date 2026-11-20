use crate::error::{Error, Result};
use crate::io::Reader;
use crate::pixels::Depth;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Version {
    #[default]
    Psd,
    Psb,
}

impl Version {
    pub fn max_dimension(self) -> u32 {
        match self {
            Version::Psd => 30_000,
            Version::Psb => 300_000,
        }
    }

    pub fn check_dimensions(self, width: u32, height: u32) -> Result<()> {
        let limit = self.max_dimension();
        if width == 0 || height == 0 || width > limit || height > limit {
            return Err(Error::TooLarge {
                width,
                height,
                limit,
            });
        }
        Ok(())
    }

    pub(crate) fn code(self) -> u16 {
        match self {
            Version::Psd => 1,
            Version::Psb => 2,
        }
    }

    pub fn is_large(self) -> bool {
        self == Version::Psb
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ColorMode {
    Bitmap,
    Grayscale,
    Indexed,
    #[default]
    Rgb,
    Cmyk,
    Multichannel,
    Duotone,
    Lab,
}

impl ColorMode {
    pub fn from_code(code: u16) -> Option<Self> {
        Some(match code {
            0 => ColorMode::Bitmap,
            1 => ColorMode::Grayscale,
            2 => ColorMode::Indexed,
            3 => ColorMode::Rgb,
            4 => ColorMode::Cmyk,
            7 => ColorMode::Multichannel,
            8 => ColorMode::Duotone,
            9 => ColorMode::Lab,
            _ => return None,
        })
    }

    pub fn code(self) -> u16 {
        match self {
            ColorMode::Bitmap => 0,
            ColorMode::Grayscale => 1,
            ColorMode::Indexed => 2,
            ColorMode::Rgb => 3,
            ColorMode::Cmyk => 4,
            ColorMode::Multichannel => 7,
            ColorMode::Duotone => 8,
            ColorMode::Lab => 9,
        }
    }

    pub fn color_channels(self) -> usize {
        match self {
            ColorMode::Rgb | ColorMode::Lab => 3,
            ColorMode::Cmyk => 4,
            _ => 1,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ColorMode::Bitmap => "bitmap",
            ColorMode::Grayscale => "grayscale",
            ColorMode::Indexed => "indexed",
            ColorMode::Rgb => "RGB",
            ColorMode::Cmyk => "CMYK",
            ColorMode::Multichannel => "multichannel",
            ColorMode::Duotone => "duotone",
            ColorMode::Lab => "Lab",
        }
    }

    pub fn supports_layers(self) -> bool {
        !matches!(
            self,
            ColorMode::Bitmap | ColorMode::Indexed | ColorMode::Multichannel
        )
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ColorModeData {
    #[default]
    Empty,
    Palette(Vec<[u8; 3]>),
    Raw(Vec<u8>),
}

impl ColorModeData {
    pub fn palette(&self) -> Option<&[[u8; 3]]> {
        match self {
            ColorModeData::Palette(palette) => Some(palette),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalMask {
    pub overlay_color_space: u16,
    pub color: [u16; 4],
    pub opacity: u16,
    pub kind: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileHeader {
    pub version: Version,
    pub channels: u16,
    pub width: u32,
    pub height: u32,
    pub depth: Depth,
    pub color_mode: ColorMode,
}

impl FileHeader {
    pub const LEN: usize = 26;
    pub const SIGNATURE: &'static [u8; 4] = b"8BPS";

    pub fn is_photoshop(bytes: &[u8]) -> bool {
        bytes.starts_with(Self::SIGNATURE)
    }

    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    pub fn composite_bytes(&self) -> u64 {
        self.depth.plane_bytes(self.width, self.height) * u64::from(self.channels)
    }

    pub fn read(bytes: &[u8]) -> Result<FileHeader> {
        let mut reader = Reader::new(bytes);
        if reader.remaining() < Self::LEN || reader.key()?.0 != *Self::SIGNATURE {
            return Err(Error::NotPhotoshop);
        }
        let version = match reader.u16()? {
            1 => Version::Psd,
            2 => Version::Psb,
            other => return Err(Error::UnsupportedVersion(other)),
        };
        reader.skip(6)?;
        let channels = reader.u16()?;
        let height = reader.u32()?;
        let width = reader.u32()?;
        let bits = reader.u16()?;
        let mode = reader.u16()?;
        let depth = Depth::from_bits(bits).ok_or(Error::Invalid {
            what: "bit depth",
            value: i64::from(bits),
        })?;
        let color_mode = ColorMode::from_code(mode).ok_or(Error::Invalid {
            what: "colour mode",
            value: i64::from(mode),
        })?;
        if !(1..=56).contains(&channels) {
            return Err(Error::Invalid {
                what: "channel count",
                value: i64::from(channels),
            });
        }
        version.check_dimensions(width, height)?;
        Ok(FileHeader {
            version,
            channels,
            width,
            height,
            depth,
            color_mode,
        })
    }
}
