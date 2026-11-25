use std::sync::Arc;

use crate::id::{EmojiId, RoleId, SoundId, StickerId};
use crate::manage::space::ImageUpload;
use crate::model::message::StickerFormat;
use crate::model::picture::Picture;
use crate::model::rich::Emoji;
use crate::model::user::User;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceEmoji {
    pub id: EmojiId,
    pub name: String,
    pub animated: bool,
    pub picture: Picture,
    pub roles: Vec<RoleId>,
    pub creator: Option<User>,
    pub available: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceSticker {
    pub id: StickerId,
    pub name: String,
    pub description: Option<String>,
    pub tags: String,
    pub format: StickerFormat,
    pub picture: Picture,
    pub creator: Option<User>,
    pub available: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpaceSound {
    pub id: SoundId,
    pub name: String,
    pub volume: f32,
    pub emoji: Option<Emoji>,
    pub creator: Option<User>,
    pub available: bool,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmojiDraft {
    pub name: String,
    pub image: ImageUpload,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StickerDraft {
    pub name: String,
    pub description: String,
    pub tags: String,
    pub file_name: String,
    pub mime: String,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundDraft {
    pub name: String,
    pub volume: f32,
    pub emoji: Option<Emoji>,
    pub mime: String,
    pub bytes: Arc<[u8]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpressionSlots {
    pub emoji: u32,
    pub stickers: u32,
    pub sounds: u32,
}

impl ExpressionSlots {
    #[must_use]
    pub const fn for_tier(tier: u8) -> ExpressionSlots {
        match tier {
            1 => ExpressionSlots {
                emoji: 100,
                stickers: 15,
                sounds: 24,
            },
            2 => ExpressionSlots {
                emoji: 150,
                stickers: 30,
                sounds: 36,
            },
            3 => ExpressionSlots {
                emoji: 250,
                stickers: 60,
                sounds: 48,
            },
            _ => ExpressionSlots {
                emoji: 50,
                stickers: 5,
                sounds: 8,
            },
        }
    }
}
