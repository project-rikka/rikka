mod capabilities;
mod convert;
mod device;
mod encoder;
mod error;
mod picture;
mod profile;
mod ring;
mod session;
mod syntax;

use std::fmt;

pub use encoder::{Encoded, Encoder, EncoderBuilder, Quality, Rate, Request};
pub use error::{Error, Result};
pub use rikka_dmabuf_convert::Imported;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Codec {
    H264,
    H265,
}

impl fmt::Display for Codec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Codec::H264 => "H.264",
            Codec::H265 => "H.265",
        })
    }
}
