mod animation;
mod chunk;
mod decoder;
mod encoder;
mod error;
mod expand;
mod header;
mod layout;
mod probe;
mod text;
mod unfilter;

pub use animation::{Animation, AnimationFrame, Blend, Dispose, FrameControl};
pub use decoder::{Decoded, Decoder};
pub use encoder::{Encoder, Metadata, Pixels};
pub use error::{ChunkKind, Error, Result};
pub use layout::{Layout, Limits, Picture, Samples};
pub use probe::{Info, probe};
pub use text::Text;

pub use rikka_deflate::Level;
