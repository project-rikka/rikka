use serde::Deserialize;

use crate::flex;
use crate::preloaded::Coverage;

pub mod level {
    pub const ALL: u8 = 0;
    pub const MENTIONS: u8 = 1;
    pub const INHERIT: u8 = 3;
}

fn inherit() -> u8 {
    level::INHERIT
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct GuildSettings {
    #[serde(deserialize_with = "flex::id")]
    pub guild_id: Option<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub muted: bool,
    pub mute_config: Option<MuteConfig>,
    #[serde(default = "inherit")]
    pub message_notifications: u8,
    #[serde(deserialize_with = "flex::flag")]
    pub suppress_everyone: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub suppress_roles: bool,
    #[serde(deserialize_with = "flex::list")]
    pub channel_overrides: Vec<ChannelOverride>,
}

impl Default for GuildSettings {
    fn default() -> GuildSettings {
        GuildSettings {
            guild_id: None,
            muted: false,
            mute_config: None,
            message_notifications: level::INHERIT,
            suppress_everyone: false,
            suppress_roles: false,
            channel_overrides: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct ChannelOverride {
    pub channel_id: String,
    #[serde(deserialize_with = "flex::flag")]
    pub muted: bool,
    pub mute_config: Option<MuteConfig>,
    #[serde(default = "inherit")]
    pub message_notifications: u8,
}

impl Default for ChannelOverride {
    fn default() -> ChannelOverride {
        ChannelOverride {
            channel_id: String::new(),
            muted: false,
            mute_config: None,
            message_notifications: level::INHERIT,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MuteConfig {
    pub end_time: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReadState {
    pub id: String,
    pub read_state_type: u8,
    #[serde(deserialize_with = "flex::id")]
    pub last_message_id: Option<String>,
    pub mention_count: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Ack {
    pub channel_id: String,
    #[serde(deserialize_with = "flex::id")]
    pub message_id: Option<String>,
    pub mention_count: Option<u32>,
    #[serde(deserialize_with = "flex::flag")]
    pub manual: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct UnreadUpdate {
    pub guild_id: Option<String>,
    #[serde(deserialize_with = "flex::list")]
    pub channel_unread_updates: Vec<ChannelUnread>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ChannelUnread {
    pub id: String,
    #[serde(deserialize_with = "flex::id")]
    pub last_message_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SettingsProto {
    pub settings: ProtoBlob,
    #[serde(rename = "partial", deserialize_with = "coverage")]
    pub coverage: Coverage,
}

fn coverage<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Coverage, D::Error> {
    Ok(match flex::flag(deserializer)? {
        true => Coverage::Partial,
        false => Coverage::Complete,
    })
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ProtoBlob {
    #[serde(rename = "type")]
    pub kind: u8,
    pub proto: String,
}
