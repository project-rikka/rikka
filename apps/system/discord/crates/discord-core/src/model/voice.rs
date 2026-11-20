use std::str::FromStr;

use crate::id::{ChannelId, SpaceId, UserId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceState {
    pub user: UserId,
    pub space: Option<SpaceId>,
    pub channel: ChannelId,
    pub self_muted: bool,
    pub self_deafened: bool,
    pub muted: bool,
    pub deafened: bool,
    pub suppressed: bool,
    pub streaming: bool,
    pub video: bool,
}

impl VoiceState {
    #[must_use]
    pub fn silenced(&self) -> bool {
        self.self_muted || self.muted || self.suppressed
    }

    #[must_use]
    pub fn unhearing(&self) -> bool {
        self.self_deafened || self.deafened
    }

    #[must_use]
    pub fn stream(&self) -> Option<StreamKey> {
        self.streaming.then_some(StreamKey {
            space: self.space,
            channel: self.channel,
            owner: self.user,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StreamKey {
    pub space: Option<SpaceId>,
    pub channel: ChannelId,
    pub owner: UserId,
}

impl std::fmt::Display for StreamKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.space {
            Some(space) => write!(formatter, "guild:{space}:{}:{}", self.channel, self.owner),
            None => write!(formatter, "call:{}:{}", self.channel, self.owner),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0:?} is not a stream key")]
pub struct NotAStreamKey(pub String);

impl FromStr for StreamKey {
    type Err = NotAStreamKey;

    fn from_str(text: &str) -> Result<StreamKey, NotAStreamKey> {
        let refused = || NotAStreamKey(text.to_owned());
        let parts: Vec<&str> = text.split(':').collect();
        let number = |part: &str| part.parse::<u64>().map_err(|_| refused());
        match parts.as_slice() {
            ["guild", space, channel, owner] => Ok(StreamKey {
                space: Some(SpaceId::new(number(space)?)),
                channel: ChannelId::new(number(channel)?),
                owner: UserId::new(number(owner)?),
            }),
            ["call", channel, owner] => Ok(StreamKey {
                space: None,
                channel: ChannelId::new(number(channel)?),
                owner: UserId::new(number(owner)?),
            }),
            _ => Err(refused()),
        }
    }
}
