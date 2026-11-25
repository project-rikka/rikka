use serde::Deserialize;

use crate::channel::Channel;
use crate::guild::Guild;
use crate::user::User;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Invite {
    pub code: String,
    pub guild_id: Option<String>,
    pub guild: Option<Guild>,
    pub channel: Option<Channel>,
    pub inviter: Option<User>,
    pub uses: u32,
    pub max_uses: u32,
    pub max_age: u32,
    pub temporary: bool,
    pub expires_at: Option<String>,
}

impl Invite {
    #[must_use]
    pub fn space(&self) -> Option<&str> {
        self.guild_id
            .as_deref()
            .or_else(|| self.guild.as_ref().map(|guild| guild.id.as_str()))
    }
}
