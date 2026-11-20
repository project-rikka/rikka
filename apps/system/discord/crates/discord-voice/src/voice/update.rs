use std::time::Duration;

use discord_core::{ChannelId, Privacy, UserId};
use futures_channel::mpsc;

#[derive(Clone, Debug, PartialEq)]
pub enum VoiceUpdate {
    Connected(Privacy),
    Privacy(Privacy),
    Speaking { user: UserId, on: bool },
    SelfSpeaking(bool),
    Latency(Duration),
    Moved(ChannelId),
    DeviceLost,
    Ended(Option<discord_core::Error>),
}

pub type VoiceUpdates = mpsc::UnboundedReceiver<VoiceUpdate>;
