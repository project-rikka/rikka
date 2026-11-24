use std::path::PathBuf;
use std::sync::Arc;

use crate::id::{ChannelId, MessageId, SpaceId, StickerId};
use crate::model::channel::Channel;
use crate::model::message::Message;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpaceTemplate {
    Blank,
    Friends,
    StudyGroup,
    SchoolClub,
    LocalCommunity,
    Creators,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceDraft {
    pub name: String,
    pub png_icon: Option<Arc<[u8]>>,
    pub template: SpaceTemplate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub upload_bytes: u64,
    pub message_chars: usize,
    pub uploads_per_message: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Focus {
    pub space: Option<SpaceId>,
    pub channel: Option<ChannelId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Latest,
    Before(MessageId),
    After(MessageId),
    Around(MessageId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft {
    pub nonce: MessageId,
    pub text: String,
    pub reply: Option<ReplyTo>,
    pub uploads: Vec<Upload>,
    pub stickers: Vec<StickerId>,
    pub silent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReplyTo {
    pub message: MessageId,
    pub mention: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upload {
    pub name: String,
    pub source: UploadSource,
    pub size: u64,
    pub mime: Option<String>,
    pub description: Option<String>,
    pub spoiler: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub enum UploadSource {
    Path(PathBuf),
    Bytes(Arc<[u8]>),
}

impl std::fmt::Debug for UploadSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadSource::Path(path) => formatter.debug_tuple("Path").field(path).finish(),
            UploadSource::Bytes(bytes) => write!(formatter, "Bytes({} bytes)", bytes.len()),
        }
    }
}

pub type Progress = Arc<dyn Fn(u64, u64) + Send + Sync>;

#[must_use]
pub fn unwatched() -> Progress {
    Arc::new(|_, _| {})
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForumPage {
    pub posts: Vec<ForumPost>,
    pub more: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumPost {
    pub thread: Channel,
    pub starter: Option<Message>,
    pub replies: u32,
}
