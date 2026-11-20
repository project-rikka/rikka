use std::time::{Duration, Instant};

use futures_channel::mpsc;

use crate::error::Error;
use crate::id::{ChannelId, UserId};
use crate::video::VideoFrame;

pub const SAMPLE_RATE: u32 = 48_000;

pub const VOICE_CHANNELS: u16 = 2;

const VOICE_FRAME: Duration = Duration::from_millis(20);

pub const VOICE_BITRATE: u32 = 64_000;

pub const VOICE_FRAME_SAMPLES: usize =
    SAMPLE_RATE as usize * VOICE_FRAME.as_millis() as usize / 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoicePacket {
    pub source: u32,
    pub timestamp: u32,
    pub arrived: Instant,
    pub opus: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoiceEvent {
    Connected { encrypted: Privacy },
    Privacy(Privacy),
    Source { source: u32, user: UserId },
    SourceGone { source: u32 },
    Packet(VoicePacket),
    Speaking { user: UserId, on: bool },
    VideoSource { source: u32, user: UserId },
    VideoGone { source: u32 },
    Video(VideoFrame),
    Wanted(u8),
    KeyframeWanted,
    KeyframeEvery(Duration),
    Bandwidth(u32),
    Viewers(Vec<UserId>),
    Paused(bool),
    Latency(Duration),
    Moved(ChannelId),
    Ended(Option<Error>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Privacy {
    Transport,
    EndToEnd { code: Option<String> },
}

pub type VoiceEvents = mpsc::UnboundedReceiver<VoiceEvent>;

#[derive(Clone, Debug)]
pub struct VoiceSink(mpsc::UnboundedSender<VoiceEvent>);

impl VoiceSink {
    #[must_use]
    pub fn new() -> (VoiceSink, VoiceEvents) {
        let (sender, receiver) = mpsc::unbounded();
        (VoiceSink(sender), receiver)
    }

    pub fn send(&self, event: VoiceEvent) {
        let _ = self.0.unbounded_send(event);
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}
