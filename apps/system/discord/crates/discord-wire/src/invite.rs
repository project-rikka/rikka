use serde::Deserialize;

use crate::channel::Channel;
use crate::guild::Guild;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Invite {
    pub code: String,
    pub guild_id: Option<String>,
    pub guild: Option<Guild>,
    pub channel: Option<Channel>,
}

impl Invite {
    #[must_use]
    pub fn space(&self) -> Option<&str> {
        self.guild_id
            .as_deref()
            .or_else(|| self.guild.as_ref().map(|guild| guild.id.as_str()))
    }
}
