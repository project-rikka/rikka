use std::sync::Arc;

use crate::id::{ChannelId, MessageId, StickerId, UserId};
use crate::model::picture::Picture;
use crate::model::rich::{Emoji, Rich};
use crate::model::user::User;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: MessageId,
    pub channel: ChannelId,
    pub author: User,
    pub at: i64,
    pub edited: Option<i64>,
    pub kind: MessageKind,
    pub text: String,
    pub content: Rich,
    pub attachments: Vec<Attachment>,
    pub embeds: Vec<Embed>,
    pub stickers: Vec<Sticker>,
    pub reactions: Vec<Reaction>,
    pub reply: Option<Reply>,
    pub mentioned: Vec<User>,
    pub mentions_me: bool,
    pub pinned: bool,
    pub thread: Option<ChannelId>,
    pub nonce: Option<MessageId>,
    pub delivery: Delivery,
}

impl Message {
    #[must_use]
    pub fn is_from(&self, user: &UserId) -> bool {
        &self.author.id == user
    }

    #[must_use]
    pub fn is_system(&self) -> bool {
        matches!(self.kind, MessageKind::System(_))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MessageKind {
    Normal,
    System(SystemEvent),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SystemEvent {
    Joined,
    Boosted,
    Pinned,
    Call { ended: Option<i64> },
    Renamed(String),
    RecipientAdded(UserId),
    RecipientRemoved(UserId),
    ThreadCreated(String),
    Other(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Delivery {
    #[default]
    Delivered,
    Sending,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    pub message: MessageId,
    pub channel: ChannelId,
    pub quoted: Option<Arc<Quoted>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quoted {
    pub author: User,
    pub excerpt: Rich,
    pub has_attachments: bool,
    pub mentions_author: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub size: u64,
    pub url: String,
    pub preview: Option<Picture>,
    pub kind: AttachmentKind,
    pub description: Option<String>,
    pub spoiler: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttachmentKind {
    Image { width: u32, height: u32 },
    Video { width: u32, height: u32 },
    Audio { millis: Option<u64> },
    File { mime: Option<String> },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Embed {
    pub kind: EmbedKind,
    pub url: Option<String>,
    pub color: Option<u32>,
    pub provider: Option<String>,
    pub author: Option<EmbedLine>,
    pub title: Option<String>,
    pub description: Rich,
    pub fields: Vec<EmbedField>,
    pub image: Option<EmbedMedia>,
    pub thumbnail: Option<EmbedMedia>,
    pub video: Option<EmbedMedia>,
    pub footer: Option<EmbedLine>,
    pub timestamp: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EmbedKind {
    #[default]
    Rich,
    Image,
    Video,
    Gif,
    Link,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedLine {
    pub text: String,
    pub url: Option<String>,
    pub icon: Option<Picture>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedField {
    pub name: Rich,
    pub value: Rich,
    pub inline: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmbedMedia {
    pub picture: Picture,
    pub url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reaction {
    pub emoji: Emoji,
    pub count: u32,
    pub me: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sticker {
    pub id: StickerId,
    pub name: String,
    pub format: StickerFormat,
    pub picture: Option<Picture>,
    pub description: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StickerFormat {
    Png,
    Apng,
    Gif,
    Lottie,
}
