use crate::format::Format;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error("FFmpeg is unavailable: {0}")]
    Unavailable(String),

    #[error("FFmpeg has no {0} decoder")]
    NoDecoder(&'static str),

    #[error("FFmpeg has no {0} encoder")]
    NoEncoder(&'static str),

    #[error("{0} file ends early")]
    Truncated(Format),

    #[error("{format}: malformed {part}")]
    Malformed { format: Format, part: &'static str },

    #[error("{format}: {feature} is not supported")]
    Unsupported {
        format: Format,
        feature: &'static str,
    },

    #[error("{width}x{height} image exceeds the decoding limits")]
    LimitsExceeded { width: u32, height: u32 },

    #[error("invalid image size {width}x{height}")]
    InvalidSize { width: u32, height: u32 },

    #[error("pixel data has {len} bytes, expected {expected}")]
    PixelDataSize { len: usize, expected: usize },

    #[error("FFmpeg could not decode {format}: {message}")]
    Decoding { format: Format, message: String },

    #[error("FFmpeg could not encode {format}: {message}")]
    Encoding { format: Format, message: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub(crate) fn malformed(format: Format, part: &'static str) -> Error {
        Error::Malformed { format, part }
    }

    pub(crate) fn unsupported(format: Format, feature: &'static str) -> Error {
        Error::Unsupported { format, feature }
    }

    pub(crate) fn decoding(format: Format, error: rikka_ffmpeg::Error) -> Error {
        match error {
            rikka_ffmpeg::Error::Unavailable(reason) => Error::Unavailable(reason),
            other => Error::Decoding {
                format,
                message: other.to_string(),
            },
        }
    }

    pub(crate) fn encoding(format: Format, error: rikka_ffmpeg::Error) -> Error {
        match error {
            rikka_ffmpeg::Error::Unavailable(reason) => Error::Unavailable(reason),
            other => Error::Encoding {
                format,
                message: other.to_string(),
            },
        }
    }
}
