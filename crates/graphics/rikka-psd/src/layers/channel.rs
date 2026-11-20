use crate::compression::{Compression, Encoded};
use crate::pixels::Plane;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ChannelId {
    Color(u16),
    Transparency,
    UserMask,
    RealUserMask,
    Other(i16),
}

impl ChannelId {
    pub fn from_code(code: i16) -> Self {
        match code {
            0.. => ChannelId::Color(code as u16),
            -1 => ChannelId::Transparency,
            -2 => ChannelId::UserMask,
            -3 => ChannelId::RealUserMask,
            other => ChannelId::Other(other),
        }
    }

    pub fn code(self) -> i16 {
        match self {
            ChannelId::Color(index) => index as i16,
            ChannelId::Transparency => -1,
            ChannelId::UserMask => -2,
            ChannelId::RealUserMask => -3,
            ChannelId::Other(code) => code,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    pub id: ChannelId,
    pub data: Encoded,
}

impl Channel {
    pub fn new(id: ChannelId, plane: &Plane, compression: Compression) -> Self {
        Self {
            id,
            data: Encoded::encode(plane, compression),
        }
    }
}
