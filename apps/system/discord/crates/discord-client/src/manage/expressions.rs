use discord_core::{
    Emoji, EmojiDraft, EmojiId, RoleId, SoundDraft, SoundId, SpaceEmoji, SpaceId, SpaceSound,
    SpaceSticker, StickerDraft, StickerId,
};
use discord_translate as translate;
use discord_wire as wire;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{data_uri, snowflakes};
use crate::client::Client;
use crate::error::Error;
use crate::rest::FormField;

#[derive(Deserialize)]
struct Sounds {
    items: Vec<wire::Sound>,
}

fn sound_emoji(emoji: Option<&Emoji>) -> (Value, Value) {
    match emoji {
        Some(Emoji { id: Some(id), .. }) => (json!(id.to_string()), Value::Null),
        Some(emoji) => (Value::Null, json!(emoji.name)),
        None => (Value::Null, Value::Null),
    }
}

impl Client {
    pub async fn space_emojis(&self, space: &SpaceId) -> discord_core::Result<Vec<SpaceEmoji>> {
        let space = space.to_string();
        self.work(move |session| async move {
            let emojis: Vec<wire::Emoji> = session
                .rest()
                .get(format!("/guilds/{space}/emojis"))
                .parse()
                .await?;
            Ok(emojis.iter().filter_map(translate::space_emoji).collect())
        })
        .await
    }

    pub async fn create_emoji(
        &self,
        space: &SpaceId,
        draft: EmojiDraft,
    ) -> discord_core::Result<SpaceEmoji> {
        let space = space.to_string();
        self.work(move |session| async move {
            let emoji: wire::Emoji = session
                .rest()
                .post(format!("/guilds/{space}/emojis"))
                .json(json!({
                    "name": draft.name,
                    "image": data_uri(&draft.image.mime, &draft.image.bytes),
                    "roles": [],
                }))
                .parse()
                .await?;
            translate::space_emoji(&emoji)
                .ok_or_else(|| Error::Unreadable("Discord answered with an unnamed emoji".into()))
        })
        .await
    }

    pub async fn edit_emoji(
        &self,
        space: &SpaceId,
        emoji: &EmojiId,
        name: &str,
        roles: Vec<RoleId>,
    ) -> discord_core::Result<()> {
        let (space, emoji, name) = (space.to_string(), emoji.to_string(), name.to_owned());
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/emojis/{emoji}"))
                .json(json!({ "name": name, "roles": snowflakes(&roles) }))
                .done()
                .await
        })
        .await
    }

    pub async fn delete_emoji(&self, space: &SpaceId, emoji: &EmojiId) -> discord_core::Result<()> {
        let (space, emoji) = (space.to_string(), emoji.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/guilds/{space}/emojis/{emoji}"))
                .done()
                .await
        })
        .await
    }

    pub async fn space_stickers(&self, space: &SpaceId) -> discord_core::Result<Vec<SpaceSticker>> {
        let space = space.to_string();
        self.work(move |session| async move {
            let stickers: Vec<wire::Sticker> = session
                .rest()
                .get(format!("/guilds/{space}/stickers"))
                .parse()
                .await?;
            Ok(stickers.iter().map(translate::space_sticker).collect())
        })
        .await
    }

    pub async fn create_sticker(
        &self,
        space: &SpaceId,
        draft: StickerDraft,
    ) -> discord_core::Result<SpaceSticker> {
        let space = space.to_string();
        self.work(move |session| async move {
            let fields = vec![
                ("name".to_owned(), FormField::Text(draft.name)),
                ("description".to_owned(), FormField::Text(draft.description)),
                ("tags".to_owned(), FormField::Text(draft.tags)),
                (
                    "file".to_owned(),
                    FormField::File {
                        name: draft.file_name,
                        mime: draft.mime,
                        bytes: rikka_net::Bytes::copy_from_slice(&draft.bytes),
                    },
                ),
            ];
            let sticker: wire::Sticker = session
                .rest()
                .post(format!("/guilds/{space}/stickers"))
                .form(fields)
                .parse()
                .await?;
            Ok(translate::space_sticker(&sticker))
        })
        .await
    }

    pub async fn edit_sticker(
        &self,
        space: &SpaceId,
        sticker: &StickerId,
        name: &str,
        description: &str,
        tags: &str,
    ) -> discord_core::Result<()> {
        let (space, sticker) = (space.to_string(), sticker.to_string());
        let body = json!({ "name": name, "description": description, "tags": tags });
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/stickers/{sticker}"))
                .json(body)
                .done()
                .await
        })
        .await
    }

    pub async fn delete_sticker(
        &self,
        space: &SpaceId,
        sticker: &StickerId,
    ) -> discord_core::Result<()> {
        let (space, sticker) = (space.to_string(), sticker.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/guilds/{space}/stickers/{sticker}"))
                .done()
                .await
        })
        .await
    }

    pub async fn space_sounds(&self, space: &SpaceId) -> discord_core::Result<Vec<SpaceSound>> {
        let space = space.to_string();
        self.work(move |session| async move {
            let sounds: Sounds = session
                .rest()
                .get(format!("/guilds/{space}/soundboard-sounds"))
                .parse()
                .await?;
            Ok(sounds.items.iter().map(translate::space_sound).collect())
        })
        .await
    }

    pub async fn create_sound(
        &self,
        space: &SpaceId,
        draft: SoundDraft,
    ) -> discord_core::Result<SpaceSound> {
        let space = space.to_string();
        let (emoji_id, emoji_name) = sound_emoji(draft.emoji.as_ref());
        self.work(move |session| async move {
            let sound: wire::Sound = session
                .rest()
                .post(format!("/guilds/{space}/soundboard-sounds"))
                .json(json!({
                    "name": draft.name,
                    "sound": data_uri(&draft.mime, &draft.bytes),
                    "volume": draft.volume,
                    "emoji_id": emoji_id,
                    "emoji_name": emoji_name,
                }))
                .parse()
                .await?;
            Ok(translate::space_sound(&sound))
        })
        .await
    }

    pub async fn edit_sound(
        &self,
        space: &SpaceId,
        sound: &SoundId,
        name: &str,
        volume: f32,
        emoji: Option<Emoji>,
    ) -> discord_core::Result<()> {
        let (space, sound) = (space.to_string(), sound.to_string());
        let (emoji_id, emoji_name) = sound_emoji(emoji.as_ref());
        let body = json!({
            "name": name,
            "volume": volume,
            "emoji_id": emoji_id,
            "emoji_name": emoji_name,
        });
        self.work(move |session| async move {
            session
                .rest()
                .patch(format!("/guilds/{space}/soundboard-sounds/{sound}"))
                .json(body)
                .done()
                .await
        })
        .await
    }

    pub async fn delete_sound(&self, space: &SpaceId, sound: &SoundId) -> discord_core::Result<()> {
        let (space, sound) = (space.to_string(), sound.to_string());
        self.work(move |session| async move {
            session
                .rest()
                .delete(format!("/guilds/{space}/soundboard-sounds/{sound}"))
                .done()
                .await
        })
        .await
    }
}
