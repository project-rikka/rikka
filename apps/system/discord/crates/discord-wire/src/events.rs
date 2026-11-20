use serde::Deserialize;

use crate::channel::Channel;
use crate::flex;
use crate::guild::{Emoji, Member, Role, Sticker};
use crate::settings::ChannelUnread;
use crate::user::{Presence, User};
use crate::voice::VoiceState;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct GuildDelete {
    pub id: String,
    #[serde(deserialize_with = "flex::flag")]
    pub unavailable: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThreadListSync {
    pub guild_id: String,
    #[serde(deserialize_with = "flex::list")]
    pub threads: Vec<Channel>,
    #[serde(deserialize_with = "flex::list")]
    pub members: Vec<ThreadMember>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThreadMember {
    pub id: Option<String>,
    pub user_id: Option<String>,
    pub guild_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThreadMembersUpdate {
    pub id: String,
    pub guild_id: Option<String>,
    pub member_count: Option<u32>,
    #[serde(deserialize_with = "flex::list")]
    pub added_members: Vec<ThreadMember>,
    #[serde(deserialize_with = "flex::list")]
    pub removed_member_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct GuildRole {
    pub guild_id: String,
    pub role: Role,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct GuildRoleDelete {
    pub guild_id: String,
    pub role_id: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MemberRemove {
    pub guild_id: String,
    pub user: User,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MembersChunk {
    pub guild_id: String,
    #[serde(deserialize_with = "flex::list")]
    pub members: Vec<Member>,
    #[serde(deserialize_with = "flex::list")]
    pub presences: Vec<Presence>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct GuildEmojis {
    pub guild_id: String,
    #[serde(deserialize_with = "flex::list")]
    pub emojis: Vec<Emoji>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct GuildStickers {
    pub guild_id: String,
    #[serde(deserialize_with = "flex::list")]
    pub stickers: Vec<Sticker>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MessageDelete {
    pub id: String,
    pub channel_id: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MessageDeleteBulk {
    #[serde(deserialize_with = "flex::list")]
    pub ids: Vec<String>,
    pub channel_id: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReactionEvent {
    pub user_id: String,
    pub channel_id: String,
    pub message_id: String,
    pub emoji: Option<Emoji>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReactionMany {
    pub channel_id: String,
    pub message_id: String,
    #[serde(deserialize_with = "flex::list")]
    pub reactions: Vec<ReactionUsers>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReactionUsers {
    pub emoji: Emoji,
    #[serde(deserialize_with = "flex::list")]
    pub users: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Typing {
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub user_id: String,
    pub member: Option<Member>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct RelationshipRemove {
    pub id: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ChannelRecipient {
    pub channel_id: String,
    pub user: User,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MemberListUpdate {
    pub id: String,
    pub guild_id: String,
    #[serde(deserialize_with = "flex::list")]
    pub groups: Vec<ListGroup>,
    #[serde(deserialize_with = "flex::list")]
    pub ops: Vec<ListOp>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ListGroup {
    pub id: String,
    pub count: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListOp {
    pub op: String,
    pub range: Option<[u32; 2]>,
    #[serde(deserialize_with = "flex::list")]
    pub items: Vec<ListItem>,
    pub index: Option<u32>,
    pub item: Option<ListItem>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ListItem {
    pub group: Option<ListGroup>,
    pub member: Option<Member>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PassiveUpdate {
    pub guild_id: String,
    #[serde(deserialize_with = "flex::list")]
    pub channels: Vec<ChannelUnread>,
    #[serde(deserialize_with = "flex::list")]
    pub updated_channels: Vec<ChannelUnread>,
    #[serde(deserialize_with = "flex::list")]
    pub voice_states: Vec<VoiceState>,
    #[serde(deserialize_with = "flex::list")]
    pub updated_voice_states: Vec<VoiceState>,
    #[serde(deserialize_with = "flex::list")]
    pub removed_voice_states: Vec<String>,
    #[serde(deserialize_with = "flex::list")]
    pub members: Vec<Member>,
    #[serde(deserialize_with = "flex::list")]
    pub updated_members: Vec<Member>,
}
