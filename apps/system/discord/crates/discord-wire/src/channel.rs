use serde::Deserialize;

use crate::flex;
use crate::user::User;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Channel {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: u8,
    pub guild_id: Option<String>,
    pub name: Option<String>,
    pub topic: Option<String>,
    pub parent_id: Option<String>,
    pub position: i32,
    pub icon: Option<String>,
    #[serde(deserialize_with = "flex::list")]
    pub recipient_ids: Vec<String>,
    #[serde(deserialize_with = "flex::list")]
    pub recipients: Vec<User>,
    #[serde(deserialize_with = "flex::list")]
    pub permission_overwrites: Vec<Overwrite>,
    #[serde(deserialize_with = "flex::flag")]
    pub nsfw: bool,
    pub rate_limit_per_user: Option<u32>,
    #[serde(deserialize_with = "flex::id")]
    pub last_message_id: Option<String>,
    pub thread_metadata: Option<ThreadMetadata>,
    pub owner_id: Option<String>,
    pub message_count: Option<u32>,
    pub member_count: Option<u32>,
    #[serde(deserialize_with = "flex::snowflakes")]
    pub applied_tags: Vec<String>,
    #[serde(deserialize_with = "flex::list")]
    pub available_tags: Vec<Tag>,
    pub default_sort_order: Option<u8>,
    pub default_forum_layout: Option<u8>,
    pub default_tag_setting: Option<String>,
    pub default_reaction_emoji: Option<DefaultReaction>,
    #[serde(deserialize_with = "flex::number")]
    pub flags: u64,
    pub bitrate: Option<u32>,
    pub user_limit: Option<u32>,
    pub rtc_region: Option<String>,
    pub video_quality_mode: Option<u8>,
    pub default_auto_archive_duration: Option<u32>,
    pub default_thread_rate_limit_per_user: Option<u32>,
    pub member: Option<serde_json::Value>,
}

impl Channel {
    pub fn recipients(&self) -> Vec<String> {
        if self.recipient_ids.is_empty() {
            self.recipients.iter().map(|user| user.id.clone()).collect()
        } else {
            self.recipient_ids.clone()
        }
    }

    pub fn is_thread(&self) -> bool {
        matches!(
            self.kind,
            kind::NEWS_THREAD | kind::PUBLIC_THREAD | kind::PRIVATE_THREAD
        )
    }

    pub fn is_private(&self) -> bool {
        matches!(self.kind, kind::DIRECT | kind::GROUP)
    }
}

pub mod kind {
    pub const TEXT: u8 = 0;
    pub const DIRECT: u8 = 1;
    pub const VOICE: u8 = 2;
    pub const GROUP: u8 = 3;
    pub const CATEGORY: u8 = 4;
    pub const ANNOUNCEMENT: u8 = 5;
    pub const NEWS_THREAD: u8 = 10;
    pub const PUBLIC_THREAD: u8 = 11;
    pub const PRIVATE_THREAD: u8 = 12;
    pub const STAGE: u8 = 13;
    pub const FORUM: u8 = 15;
    pub const MEDIA: u8 = 16;
}

pub mod flags {
    pub const PINNED: u64 = 1 << 1;
    pub const REQUIRE_TAG: u64 = 1 << 4;
}

pub mod sort_order {
    pub const CREATION_DATE: u8 = 1;
}

pub mod forum_layout {
    pub const LIST_VIEW: u8 = 1;
    pub const GALLERY_VIEW: u8 = 2;
}

pub mod tag_setting {
    pub const MATCH_ALL: &str = "match_all";
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct DefaultReaction {
    #[serde(deserialize_with = "flex::id")]
    pub emoji_id: Option<String>,
    pub emoji_name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Overwrite {
    pub id: String,
    #[serde(rename = "type", deserialize_with = "flex::kind")]
    pub kind: u8,
    #[serde(deserialize_with = "flex::number")]
    pub allow: u64,
    #[serde(deserialize_with = "flex::number")]
    pub deny: u64,
}

impl Overwrite {
    pub fn is_member(&self) -> bool {
        self.kind == 1
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ThreadMetadata {
    #[serde(deserialize_with = "flex::flag")]
    pub archived: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub locked: bool,
    pub create_timestamp: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Tag {
    #[serde(deserialize_with = "flex::snowflake")]
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "flex::id")]
    pub emoji_id: Option<String>,
    pub emoji_name: Option<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub moderated: bool,
}
