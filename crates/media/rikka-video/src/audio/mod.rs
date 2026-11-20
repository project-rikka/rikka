mod codec;
mod coder;
mod config;
mod fold;
mod samples;
mod track;

pub use codec::AudioCodec;
pub(crate) use coder::SoundCoder;
pub use config::AudioConfig;
pub(crate) use samples::append_decoded;
pub(crate) use track::AudioTrack;
