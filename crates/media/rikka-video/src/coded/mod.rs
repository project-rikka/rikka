mod config;
mod muxer;
mod packet;
mod sound;
mod stream;
mod video;

pub use config::PacketConfig;
pub use muxer::Muxer;
pub(crate) use muxer::{add_stream, write_sample};
pub(crate) use packet::{MICROS, MUXED, from_packet};
pub use sound::SoundPackets;
pub use stream::StreamInfo;
pub use video::VideoPackets;
