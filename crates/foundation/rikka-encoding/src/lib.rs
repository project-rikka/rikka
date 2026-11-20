mod decoder;
mod guess;

pub use decoder::{Decoded, Decoder, decode};
pub use encoding_rs::{Encoding, SHIFT_JIS, UTF_8, UTF_16BE, UTF_16LE, WINDOWS_1252};
pub use guess::guess;
