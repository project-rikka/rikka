use serde::Deserialize;

use crate::channel::Channel;
use crate::flex;
use crate::guild::{Guild, Member};
use crate::settings::{GuildSettings, ReadState};
use crate::user::{Presence, Relationship, Session, User};
use crate::voice::VoiceState;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Ready {
    pub user: User,
    #[serde(deserialize_with = "flex::list")]
    pub guilds: Vec<Guild>,
    #[serde(deserialize_with = "flex::list")]
    pub users: Vec<User>,
    #[serde(deserialize_with = "flex::list")]
    pub merged_members: Vec<Vec<Member>>,
    #[serde(deserialize_with = "flex::list")]
    pub private_channels: Vec<Channel>,
    #[serde(deserialize_with = "flex::list")]
    pub relationships: Vec<Relationship>,
    #[serde(deserialize_with = "flex::listed")]
    pub read_state: Vec<ReadState>,
    #[serde(deserialize_with = "flex::listed")]
    pub user_guild_settings: Vec<GuildSettings>,
    pub user_settings_proto: Option<String>,
    #[serde(deserialize_with = "flex::list")]
    pub sessions: Vec<Session>,
    pub session_id: String,
    pub resume_gateway_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Supplemental {
    #[serde(deserialize_with = "flex::list")]
    pub guilds: Vec<SupplementalGuild>,
    #[serde(deserialize_with = "flex::list")]
    pub merged_members: Vec<Vec<Member>>,
    pub merged_presences: MergedPresences,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct SupplementalGuild {
    pub id: String,
    #[serde(deserialize_with = "flex::list")]
    pub voice_states: Vec<VoiceState>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MergedPresences {
    #[serde(deserialize_with = "flex::list")]
    pub friends: Vec<Presence>,
    #[serde(deserialize_with = "flex::list")]
    pub guilds: Vec<Vec<Presence>>,
}
