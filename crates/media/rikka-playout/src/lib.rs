mod buffer;
mod delay;
mod opus;
mod playout;
mod queue_target;
mod samples;
mod stretch;

pub use playout::{Decode, Packet, Playout, SpeechType};
pub use queue_target::QueueTarget;

pub const RATE: u32 = 48_000;
pub const CHANNELS: usize = 2;
pub const FRAME: usize = 960;
pub const CHUNK: usize = 480;
