use crate::bytes::{Reader, Writer};
use crate::error::DecodeError;

macro_rules! wire_enum {
    ($(#[$meta:meta])* $name:ident as $field:literal { $($variant:ident = $value:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &[$name] = &[$($name::$variant),+];

            #[must_use]
            pub const fn to_wire(self) -> u8 {
                match self {
                    $($name::$variant => $value),+
                }
            }

            pub fn from_wire(value: u8) -> Result<$name, DecodeError> {
                match value {
                    $($value => Ok($name::$variant),)+
                    other => Err(DecodeError::InvalidValue { field: $field, value: u64::from(other) }),
                }
            }
        }
    };
}

wire_enum!(Codec as "codec" { H264 = 1, H265 = 2, Av1 = 3 });
wire_enum!(Chroma as "chroma" { Yuv420 = 1, Yuv444 = 2 });
wire_enum!(Refusal as "refusal" { WrongPassword = 1, UnknownToken = 2, Throttled = 3, Unavailable = 4 });
wire_enum!(EndReason as "end reason" {
    Replaced = 1,
    Stopped = 2,
    SignedOut = 3,
    Disconnected = 4,
    Unsupported = 5,
});
wire_enum!(StreamKind as "stream kind" { Input = 1, File = 2 });
wire_enum!(Button as "button" { Primary = 0, Middle = 1, Secondary = 2, Back = 3, Forward = 4 });

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Chromas(u8);

impl Chromas {
    pub const YUV420: Chromas = Chromas(0b01);
    pub const YUV444: Chromas = Chromas(0b10);

    #[must_use]
    pub const fn with(self, other: Chromas) -> Chromas {
        Chromas(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, chroma: Chroma) -> bool {
        let bit = match chroma {
            Chroma::Yuv420 => Self::YUV420.0,
            Chroma::Yuv444 => Self::YUV444.0,
        };
        self.0 & bit != 0
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn from_bits(bits: u8) -> Chromas {
        Chromas(bits)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Depths(u8);

impl Depths {
    pub const EIGHT: Depths = Depths(0b01);
    pub const TEN: Depths = Depths(0b10);

    #[must_use]
    pub const fn with(self, other: Depths) -> Depths {
        Depths(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, depth: u8) -> bool {
        match depth {
            8 => self.0 & Self::EIGHT.0 != 0,
            10 => self.0 & Self::TEN.0 != 0,
            _ => false,
        }
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn from_bits(bits: u8) -> Depths {
        Depths(bits)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
    pub refresh_mhz: u32,
}

impl Viewport {
    pub(crate) fn write(&self, writer: &mut Writer<'_>) {
        writer
            .u32(self.width)
            .u32(self.height)
            .u32(self.refresh_mhz);
    }

    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Viewport, DecodeError> {
        Ok(Viewport {
            width: reader.u32()?,
            height: reader.u32()?,
            refresh_mhz: reader.u32()?,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decoder {
    pub codec: Codec,
    pub chromas: Chromas,
    pub depths: Depths,
    pub max_width: u16,
    pub max_height: u16,
}

impl Decoder {
    pub(crate) fn write(&self, writer: &mut Writer<'_>) {
        writer
            .u8(self.codec.to_wire())
            .u8(self.chromas.bits())
            .u8(self.depths.bits())
            .u8(0)
            .u16(self.max_width)
            .u16(self.max_height);
    }

    pub(crate) fn read(reader: &mut Reader<'_>) -> Result<Decoder, DecodeError> {
        let codec = Codec::from_wire(reader.u8()?)?;
        let chromas = Chromas::from_bits(reader.u8()?);
        let depths = Depths::from_bits(reader.u8()?);
        reader.u8()?;
        Ok(Decoder {
            codec,
            chromas,
            depths,
            max_width: reader.u16()?,
            max_height: reader.u16()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Display {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_mhz: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stream {
    pub stream: u16,
    pub codec: Codec,
    pub chroma: Chroma,
    pub depth: u8,
    pub width: u32,
    pub height: u32,
    pub frame_rate_mhz: u32,
    pub encoder: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorShape {
    pub serial: u32,
    pub width: u16,
    pub height: u16,
    pub hotspot_x: i16,
    pub hotspot_y: i16,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub target_bps: u64,
    pub estimate_bps: u64,
    pub frame_rate_mhz: u32,
    pub encode_us: u32,
    pub protection_permille: u16,
    pub loss_permille: u16,
    pub rtt_us: u32,
}
