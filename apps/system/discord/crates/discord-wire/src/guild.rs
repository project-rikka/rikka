use serde::Deserialize;

use crate::channel::Channel;
use crate::flex;
use crate::user::{Presence, User};
use crate::voice::VoiceState;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Guild {
    pub id: String,
    pub properties: Option<Properties>,
    pub name: Option<String>,
    pub icon: Option<String>,
    pub banner: Option<String>,
    pub owner_id: Option<String>,
    pub default_message_notifications: Option<u8>,
    #[serde(deserialize_with = "flex::list")]
    pub channels: Vec<Channel>,
    #[serde(deserialize_with = "flex::list")]
    pub threads: Vec<Channel>,
    #[serde(deserialize_with = "flex::list")]
    pub roles: Vec<Role>,
    #[serde(deserialize_with = "flex::list")]
    pub emojis: Vec<Emoji>,
    #[serde(deserialize_with = "flex::list")]
    pub stickers: Vec<Sticker>,
    #[serde(deserialize_with = "flex::list")]
    pub members: Vec<Member>,
    #[serde(deserialize_with = "flex::list")]
    pub presences: Vec<Presence>,
    #[serde(deserialize_with = "flex::list")]
    pub voice_states: Vec<VoiceState>,
    pub joined_at: Option<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub unavailable: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Properties {
    pub name: Option<String>,
    pub icon: Option<String>,
    pub banner: Option<String>,
    pub owner_id: Option<String>,
    pub default_message_notifications: Option<u8>,
}

impl Guild {
    pub fn properties(&self) -> Properties {
        let nested = self.properties.clone().unwrap_or_default();
        Properties {
            name: nested.name.or_else(|| self.name.clone()),
            icon: nested.icon.or_else(|| self.icon.clone()),
            banner: nested.banner.or_else(|| self.banner.clone()),
            owner_id: nested.owner_id.or_else(|| self.owner_id.clone()),
            default_message_notifications: nested
                .default_message_notifications
                .or(self.default_message_notifications),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub color: u32,
    pub colors: Option<RoleColors>,
    pub position: i32,
    #[serde(deserialize_with = "flex::flag")]
    pub hoist: bool,
    pub icon: Option<String>,
    #[serde(deserialize_with = "flex::number")]
    pub permissions: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RoleColors {
    pub primary_color: u32,
}

impl Role {
    pub fn colour(&self) -> Option<u32> {
        let primary = self
            .colors
            .as_ref()
            .map_or(self.color, |colors| colors.primary_color);
        (primary != 0).then_some(primary)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Member {
    pub user: Option<User>,
    pub user_id: Option<String>,
    pub guild_id: Option<String>,
    pub nick: Option<String>,
    pub avatar: Option<String>,
    pub banner: Option<String>,
    #[serde(deserialize_with = "flex::list")]
    pub roles: Vec<String>,
    pub joined_at: Option<String>,
    pub communication_disabled_until: Option<String>,
    pub presence: Option<Presence>,
}

impl Member {
    pub fn user_id(&self) -> Option<&str> {
        self.user
            .as_ref()
            .map(|user| user.id.as_str())
            .or(self.user_id.as_deref())
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Emoji {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub animated: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Sticker {
    pub id: String,
    pub name: String,
    pub format_type: u8,
    pub description: Option<String>,
}
