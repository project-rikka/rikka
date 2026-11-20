mod aggregate;
mod assembler;
mod av1;
mod depacketize;
mod described;
mod extensions;
mod h264;
mod h265;
mod receiver;
mod reception;
mod sender;
mod sps;
mod vp8;
mod vp9;

use discord_core::VideoCodec;

pub use assembler::Assembled;
pub use extensions::STREAM_RID;
pub use receiver::{Arrived, VideoReceiver};
pub use sender::{Plain, VideoSender};

pub const MAX_PAYLOAD: usize = 1100;

#[must_use]
pub fn packetize_h264(frame: &[u8]) -> Vec<Vec<u8>> {
    h264::packetize(frame, MAX_PAYLOAD)
}

#[must_use]
pub fn prepare_h264(frame: &[u8]) -> Vec<u8> {
    h264::prepare(frame)
}

#[must_use]
pub fn depacketize(codec: VideoCodec, payloads: &[&[u8]]) -> Option<Vec<u8>> {
    depacketize::assemble(codec, payloads)
}
