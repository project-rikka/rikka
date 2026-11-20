use discord_core::{ChannelId, UserId};

use crate::error::{Error, Result};

const GATEWAY_VERSION: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Voice,
    Broadcast,
    Watch,
}

impl Role {
    pub(crate) fn has_video(self) -> bool {
        !matches!(self, Role::Voice)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub endpoint: String,
    pub token: String,
    pub server: u64,
    pub channel: ChannelId,
    pub group: u64,
    pub user: UserId,
    pub session: String,
}

impl Endpoint {
    pub(crate) fn url(&self) -> Result<String> {
        let host = self
            .endpoint
            .trim()
            .trim_start_matches("wss://")
            .trim_end_matches('/');
        if host.is_empty() || host.contains(['/', '?', '#', ' ']) {
            return Err(Error::Endpoint(format!(
                "{:?} is not a host",
                self.endpoint
            )));
        }
        Ok(format!("wss://{host}/?v={GATEWAY_VERSION}"))
    }
}

impl std::fmt::Debug for Endpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Endpoint")
            .field("endpoint", &self.endpoint)
            .field("server", &self.server)
            .field("channel", &self.channel)
            .field("group", &self.group)
            .field("user", &self.user)
            .finish_non_exhaustive()
    }
}
