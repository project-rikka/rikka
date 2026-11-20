use serde::Deserialize;

use crate::channel::Channel;
use crate::flex;
use crate::guild::{Emoji, Member};
use crate::user::User;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Message {
    pub id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub author: Option<User>,
    pub member: Option<Member>,
    pub content: Option<String>,
    pub timestamp: Option<String>,
    pub edited_timestamp: Option<String>,
    #[serde(rename = "type")]
    pub kind: u8,
    #[serde(deserialize_with = "flex::list")]
    pub mentions: Vec<User>,
    #[serde(deserialize_with = "flex::list")]
    pub mention_roles: Vec<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub mention_everyone: bool,
    #[serde(deserialize_with = "flex::list")]
    pub attachments: Vec<Attachment>,
    pub embeds: Option<Vec<Embed>>,
    #[serde(deserialize_with = "flex::list")]
    pub sticker_items: Vec<StickerItem>,
    #[serde(deserialize_with = "flex::list")]
    pub reactions: Vec<Reaction>,
    pub message_reference: Option<Reference>,
    pub referenced_message: Option<Box<Message>>,
    #[serde(deserialize_with = "flex::list")]
    pub message_snapshots: Vec<Snapshot>,
    #[serde(deserialize_with = "flex::flag")]
    pub pinned: bool,
    pub thread: Option<Channel>,
    #[serde(deserialize_with = "flex::text")]
    pub nonce: Option<String>,
    pub flags: u64,
    pub call: Option<CallInfo>,
    pub webhook_id: Option<String>,
}

pub mod flags {
    pub const HAS_THREAD: u64 = 1 << 5;
    pub const SUPPRESS_NOTIFICATIONS: u64 = 1 << 12;
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub message: Message,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct CallInfo {
    pub ended_timestamp: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Reference {
    pub message_id: Option<String>,
    pub channel_id: Option<String>,
    #[serde(rename = "type")]
    pub kind: u8,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub url: String,
    pub proxy_url: Option<String>,
    pub content_type: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_secs: Option<f64>,
    pub description: Option<String>,
    pub flags: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Embed {
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub color: Option<u32>,
    pub timestamp: Option<String>,
    pub footer: Option<EmbedFooter>,
    pub image: Option<EmbedMedia>,
    pub thumbnail: Option<EmbedMedia>,
    pub video: Option<EmbedMedia>,
    pub provider: Option<EmbedProvider>,
    pub author: Option<EmbedAuthor>,
    #[serde(deserialize_with = "flex::list")]
    pub fields: Vec<EmbedField>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EmbedFooter {
    pub text: String,
    pub icon_url: Option<String>,
    pub proxy_icon_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EmbedMedia {
    pub url: Option<String>,
    pub proxy_url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EmbedProvider {
    pub name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EmbedAuthor {
    pub name: String,
    pub url: Option<String>,
    pub icon_url: Option<String>,
    pub proxy_icon_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    #[serde(deserialize_with = "flex::flag")]
    pub inline: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct StickerItem {
    pub id: String,
    pub name: String,
    pub format_type: u8,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Reaction {
    pub emoji: Emoji,
    pub count: u32,
    #[serde(deserialize_with = "flex::flag")]
    pub me: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub me_burst: bool,
}
