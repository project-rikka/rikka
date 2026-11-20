mod decoder;
mod encoder;
mod error;

pub use decoder::OpusDecoder;
pub use encoder::{OPUS_MAX_PACKET, OPUS_SAMPLE_RATE, OpusApplication, OpusEncoder, OpusSignal};
pub use error::{Error, Result};
