use std::path::PathBuf;

use crate::color_type::ColorType;
use crate::format::Format;
use crate::region::Region;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("unrecognized image format")]
    UnknownFormat,

    #[error("no image format for extension of \"{}\"", .0.display())]
    UnknownExtension(PathBuf),

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("{0} needs FFmpeg, which is not installed")]
    NeedsFfmpeg(Format),

    #[error("reading {format} needs FFmpeg's {codec} decoder, which the installed FFmpeg lacks")]
    NeedsDecoder { format: Format, codec: &'static str },

    #[error("writing {format} needs FFmpeg's {codec} encoder, which the installed FFmpeg lacks")]
    NeedsEncoder { format: Format, codec: &'static str },

    #[error("decoding failed: {0}")]
    Decoding(String),

    #[error("encoding failed: {0}")]
    Encoding(String),

    #[error("decoding limits exceeded: {0}")]
    LimitsExceeded(String),

    #[error("invalid image size {width}x{height}")]
    InvalidSize { width: u32, height: u32 },

    #[error("region {region} outside {width}x{height} image")]
    RegionOutOfBounds {
        region: Region,
        width: u32,
        height: u32,
    },

    #[error("{width}x{height} {color:?} pixel data needs {expected} bytes, got {len}")]
    PixelDataSize {
        len: usize,
        expected: usize,
        width: u32,
        height: u32,
        color: ColorType,
    },

    #[error("image loader stopped before decoding finished")]
    LoaderStopped,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<rikka_svg::Error> for Error {
    fn from(err: rikka_svg::Error) -> Self {
        use rikka_svg::Error as Svg;

        match err {
            Svg::Io(io) => Error::Io(io),
            Svg::InvalidSize { width, height } => Error::InvalidSize { width, height },
            over @ (Svg::TooComplex | Svg::TooManyPixels { .. }) => {
                Error::LimitsExceeded(over.to_string())
            }
            damaged => Error::Decoding(damaged.to_string()),
        }
    }
}
