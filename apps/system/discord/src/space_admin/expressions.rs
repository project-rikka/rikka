use std::future::Future;

use discord_client::Client;
use discord_core::{
    Emoji, EmojiDraft, EmojiId, ExpressionSlots, ImageUpload, Permissions, SoundDraft, SoundId,
    SpaceEmoji, SpaceId, SpaceSound, SpaceSticker, StickerDraft, StickerId,
};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{SpaceAdmin, Upload, hint, loading};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::manage::{Picked, Prompt, pick_file, pick_picture};
use crate::widgets::picture_tile;

const EMOJI_SIDE: u32 = 128;
const EMOJI_LIMIT: u64 = 256 * 1024;
const STICKER_LIMIT: u64 = 512 * 1024;
const SOUND_LIMIT: u64 = 512 * 1024;
const STICKER_TYPES: [&str; 3] = ["image/png", "image/gif", "application/json"];
const SOUND_TYPES: [&str; 2] = ["audio/mpeg", "audio/ogg"];
const PREVIEW: f32 = 40.0;
const SHORTEST_NAME: usize = 2;
const LONGEST_NAME: usize = 32;
const DEFAULT_VOLUME: f32 = 1.0;

fn emoji_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|character| match character.is_ascii_alphanumeric() {
            true => character,
            false => '_',
        })
        .take(LONGEST_NAME)
        .collect();
    match cleaned.len() < SHORTEST_NAME {
        true => format!("emoji_{cleaned}"),
        false => cleaned,
    }
}

fn tier(linked: &LinkedAccount, space: &SpaceId) -> u8 {
    linked.space(space).map_or(0, |known| known.premium_tier)
}

fn creator(user: Option<&discord_core::User>) -> Element {
    match user {
        Some(user) => text(user.name().to_owned())
            .caption()
            .muted()
            .no_wrap()
            .any(),
        None => spacer().any(),
    }
}

fn may_manage(linked: &LinkedAccount, space: &SpaceId) -> bool {
    linked
        .space(space)
        .is_some_and(|known| known.can(Permissions::MANAGE_GUILD_EXPRESSIONS))
}

pub(super) fn emoji(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let slots = ExpressionSlots::for_tier(tier(linked, space)).emoji;
    let manage = may_manage(linked, space);
    loading(&admin.emojis, |emojis| {
        let (animated, still): (Vec<&SpaceEmoji>, Vec<&SpaceEmoji>) =
            emojis.iter().partition(|emoji| emoji.animated);
        column! {
            hint(tr!("discord.admin.emoji-hint"));
            button(text(tr!("discord.admin.upload-emoji")))
                .primary()
                .on_press(|chat: &mut Chat| chat.upload_emoji());
            section(
                tr!("discord.admin.emoji-static", count = still.len() as i64, limit = i64::from(slots)),
                column_of(still.iter().map(|emoji| emoji_row(admin, emoji, manage))).gap(Space::TIGHT).fill_width(),
            );
            section(
                tr!("discord.admin.emoji-animated", count = animated.len() as i64, limit = i64::from(slots)),
                column_of(animated.iter().map(|emoji| emoji_row(admin, emoji, manage))).gap(Space::TIGHT).fill_width(),
            );
        }
        .gap(Space::LOOSE)
        .fill_width()
        .any()
    })
}

fn emoji_row(admin: &SpaceAdmin, emoji: &SpaceEmoji, manage: bool) -> Element {
    let id = emoji.id;
    let renaming = *admin.renaming == Some(id.get());
    let name = emoji.name.clone();
    row! {
        picture_tile(Some(&emoji.picture), &emoji.name, 32.0, 4.0);
        match renaming {
            true => text_field(&admin.docs.rename)
                .autofocus(true)
                .max_len(LONGEST_NAME)
                .on_submit(move |chat: &mut Chat| chat.rename_emoji(&id))
                .on_blur(|admin: &mut SpaceAdmin| admin.renaming.set(None))
                .grow(1.0)
                .any(),
            false => text(format!(":{}:", emoji.name)).label().grow(1.0).min_width(0.0).any(),
        };
        creator(emoji.creator.as_ref());
        if manage {
            button(text(tr!("discord.admin.rename")))
                .ghost()
                .small()
                .on_press(move |admin: &mut SpaceAdmin| {
                    admin.docs.rename.reset(&name);
                    admin.renaming.set(Some(id.get()));
                });
            button(icon(MaterialSymbol::DELETE).extent(Length::px(16.0)))
                .ghost()
                .small()
                .on_press(move |chat: &mut Chat| chat.ask_delete_emoji(&id));
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

pub(super) fn stickers(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let slots = ExpressionSlots::for_tier(tier(linked, space)).stickers;
    let manage = may_manage(linked, space);
    loading(&admin.stickers, |stickers| {
        column! {
            hint(tr!("discord.admin.sticker-hint"));
            if let Some(form) = upload_form(admin) {
                form;
            } else {
                button(text(tr!("discord.admin.upload-sticker")))
                    .primary()
                    .enabled(stickers.len() < slots as usize)
                    .on_press(|chat: &mut Chat| chat.pick_sticker());
            }
            section(
                tr!("discord.admin.sticker-count", count = stickers.len() as i64, limit = i64::from(slots)),
                column_of(stickers.iter().map(|sticker| sticker_row(sticker, manage))).gap(Space::TIGHT).fill_width(),
            );
        }
        .gap(Space::LOOSE)
        .fill_width()
        .any()
    })
}

fn sticker_row(sticker: &SpaceSticker, manage: bool) -> Element {
    let id = sticker.id;
    let edited = sticker.clone();
    row! {
        picture_tile(Some(&sticker.picture), &sticker.name, PREVIEW, 4.0);
        column! {
            text(sticker.name.clone()).label().semibold().no_wrap();
            text(sticker.description.clone().unwrap_or_default()).caption().muted().no_wrap();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        text(sticker.tags.clone()).label();
        creator(sticker.creator.as_ref());
        if manage {
            button(text(tr!("discord.admin.edit")))
                .ghost()
                .small()
                .on_press(move |admin: &mut SpaceAdmin| admin.edit_sticker(&edited));
            button(icon(MaterialSymbol::DELETE).extent(Length::px(16.0)))
                .ghost()
                .small()
                .on_press(move |chat: &mut Chat| chat.ask_delete_sticker(&id));
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

pub(super) fn sounds(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let slots = ExpressionSlots::for_tier(tier(linked, space)).sounds;
    let manage = may_manage(linked, space);
    loading(&admin.sounds, |sounds| {
        column! {
            hint(tr!("discord.admin.sound-hint"));
            if let Some(form) = upload_form(admin) {
                form;
            } else {
                button(text(tr!("discord.admin.upload-sound")))
                    .primary()
                    .enabled(sounds.len() < slots as usize)
                    .on_press(|chat: &mut Chat| chat.pick_sound());
            }
            section(
                tr!("discord.admin.sound-count", count = sounds.len() as i64, limit = i64::from(slots)),
                column_of(sounds.iter().map(|sound| sound_row(sound, manage))).gap(Space::TIGHT).fill_width(),
            );
        }
        .gap(Space::LOOSE)
        .fill_width()
        .any()
    })
}

fn sound_row(sound: &SpaceSound, manage: bool) -> Element {
    let id = sound.id;
    let edited = sound.clone();
    let url = sound.url.clone();
    row! {
        text(sound.emoji.as_ref().map(|emoji| emoji.name.clone()).unwrap_or_default()).label();
        text(sound.name.clone()).label().semibold().grow(1.0).min_width(0.0);
        text(format!("{}%", (sound.volume * 100.0).round() as i64)).caption().muted();
        creator(sound.creator.as_ref());
        button(icon(MaterialSymbol::PLAY_ARROW).extent(Length::px(16.0)))
            .ghost()
            .small()
            .on_press(move |chat: &mut Chat| chat.open_page(&url));
        if manage {
            button(text(tr!("discord.admin.edit")))
                .ghost()
                .small()
                .on_press(move |admin: &mut SpaceAdmin| admin.edit_sound(&edited));
            button(icon(MaterialSymbol::DELETE).extent(Length::px(16.0)))
                .ghost()
                .small()
                .on_press(move |chat: &mut Chat| chat.ask_delete_sound(&id));
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn upload_form(admin: &SpaceAdmin) -> Option<Element> {
    let upload = admin.upload.as_ref()?.clone();
    let sticker = matches!(upload, Upload::Sticker(_) | Upload::EditSticker(_));
    let volume = match &upload {
        Upload::Sound { volume, .. } | Upload::EditSound { volume, .. } => Some(*volume),
        Upload::Sticker(_) | Upload::EditSticker(_) => None,
    };
    let title = match &upload {
        Upload::Sticker(picked) | Upload::Sound { picked, .. } => picked.name.clone(),
        Upload::EditSticker(_) | Upload::EditSound { .. } => tr!("discord.admin.edit").to_string(),
    };
    Some(
        card(
            column! {
                text(title).label().semibold();
                text(tr!("discord.admin.expression-name")).caption().semibold().muted();
                text_field(&admin.docs.upload_name).max_len(LONGEST_NAME).fill_width();
                text(match sticker {
                    true => tr!("discord.admin.sticker-emoji"),
                    false => tr!("discord.admin.sound-emoji"),
                })
                .caption()
                .semibold()
                .muted();
                text_field(&admin.docs.upload_tags).max_len(LONGEST_NAME).width(160.0);
                if sticker {
                    text(tr!("discord.admin.sticker-description")).caption().semibold().muted();
                    text_field(&admin.docs.upload_description).max_len(100).fill_width();
                }
                if let Some(volume) = volume {
                    field(
                        tr!("discord.admin.sound-volume"),
                        slider(volume)
                            .range(0.0..=1.0)
                            .step(0.01)
                            .on_commit(|admin: &mut SpaceAdmin, volume: f32| admin.set_upload_volume(volume))
                            .width(200.0),
                    );
                }
                row! {
                    spacer().grow(1.0);
                    button(text(tr!("discord.manage.cancel")))
                        .ghost()
                        .on_press(|admin: &mut SpaceAdmin| admin.upload.set(None));
                    button(text(tr!("discord.manage.save")))
                        .primary()
                        .enabled(!admin.is_saving())
                        .on_press(|chat: &mut Chat| chat.submit_upload());
                }
                .gap(Space::SNUG)
                .fill_width();
            }
            .gap(Space::SNUG)
            .fill_width(),
        )
        .any(),
    )
}

impl SpaceAdmin {
    fn begin_upload(&mut self, upload: Upload, name: &str, tags: &str, description: &str) {
        self.docs.upload_name.reset(name);
        self.docs.upload_tags.reset(tags);
        self.docs.upload_description.reset(description);
        self.upload.set(Some(upload));
    }

    fn edit_sticker(&mut self, sticker: &SpaceSticker) {
        self.begin_upload(
            Upload::EditSticker(sticker.id),
            &sticker.name,
            &sticker.tags,
            sticker.description.as_deref().unwrap_or_default(),
        );
    }

    fn edit_sound(&mut self, sound: &SpaceSound) {
        let emoji = sound
            .emoji
            .as_ref()
            .map(|emoji| match emoji.id {
                Some(id) => id.to_string(),
                None => emoji.name.clone(),
            })
            .unwrap_or_default();
        self.begin_upload(
            Upload::EditSound {
                id: sound.id,
                volume: sound.volume,
            },
            &sound.name,
            &emoji,
            "",
        );
    }

    fn set_upload_volume(&mut self, volume: f32) {
        let changed = match (*self.upload).clone() {
            Some(Upload::Sound { picked, .. }) => Some(Upload::Sound { picked, volume }),
            Some(Upload::EditSound { id, .. }) => Some(Upload::EditSound { id, volume }),
            other => other,
        };
        self.upload.set(changed);
    }
}

fn store<T: PartialEq>(slot: &mut Tracked<super::Load<T>>, result: Result<T, String>) {
    slot.set(Some(result));
}

impl Chat {
    pub(super) fn load_emojis(&mut self) {
        self.fetch(
            |client, space| async move { client.space_emojis(&space).await },
            |admin, emojis| store(&mut admin.emojis, emojis),
        );
    }

    pub(super) fn load_stickers(&mut self) {
        self.fetch(
            |client, space| async move { client.space_stickers(&space).await },
            |admin, stickers| store(&mut admin.stickers, stickers),
        );
    }

    pub(super) fn load_sounds(&mut self) {
        self.fetch(
            |client, space| async move { client.space_sounds(&space).await },
            |admin, sounds| store(&mut admin.sounds, sounds),
        );
    }

    fn upload_emoji(&mut self) {
        let parent = window_identifier();
        spawn_blocking(
            move || pick_picture(parent, EMOJI_SIDE, EMOJI_LIMIT),
            |chat: &mut Chat, picked: Result<Option<(String, ImageUpload)>, String>| match picked {
                Ok(Some((name, image))) => chat.create_emoji(emoji_name(&name), image),
                Ok(None) => {}
                Err(error_message) => chat.manage_failed(error_message),
            },
        );
    }

    fn create_emoji(&mut self, name: String, image: ImageUpload) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let draft = EmojiDraft { name, image };
        self.act(
            &key,
            move |client| {
                let draft = draft.clone();
                async move { client.create_emoji(&space, draft).await }
            },
            move |chat: &mut Chat, created: SpaceEmoji| {
                if let Some(admin) = chat.admin_for(space)
                    && let Some(Ok(mut emojis)) = (*admin.emojis).clone()
                {
                    emojis.push(created);
                    admin.emojis.set(Some(Ok(emojis)));
                }
            },
        );
    }

    fn rename_emoji(&mut self, emoji: &EmojiId) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let name = emoji_name(self.space_admin().docs.rename.text().trim());
        let roles = match self.space_admin().emojis.as_ref() {
            Some(Ok(emojis)) => emojis
                .iter()
                .find(|known| known.id == *emoji)
                .map(|known| known.roles.clone())
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let emoji = *emoji;
        self.space_admin_mut().renaming.set(None);
        self.act(
            &key,
            move |client| {
                let (name, roles) = (name.clone(), roles.clone());
                async move { client.edit_emoji(&space, &emoji, &name, roles).await }
            },
            |chat: &mut Chat, ()| chat.load_emojis(),
        );
    }

    fn ask_delete_emoji(&mut self, emoji: &EmojiId) {
        let emoji = *emoji;
        self.confirm_delete(
            tr!("discord.admin.delete-emoji"),
            move |client, space| async move { client.delete_emoji(&space, &emoji).await },
            Chat::load_emojis,
        );
    }

    fn pick_sticker(&mut self) {
        let parent = window_identifier();
        spawn_blocking(
            move || {
                pick_file(
                    parent,
                    tr!("discord.admin.upload-sticker"),
                    &STICKER_TYPES,
                    STICKER_LIMIT,
                )
            },
            |chat: &mut Chat, picked: Result<Option<Picked>, String>| match picked {
                Ok(Some(picked)) => {
                    let name = picked.name.clone();
                    chat.space_admin_mut()
                        .begin_upload(Upload::Sticker(picked), &name, "", "");
                }
                Ok(None) => {}
                Err(error_message) => chat.manage_failed(error_message),
            },
        );
    }

    fn pick_sound(&mut self) {
        let parent = window_identifier();
        spawn_blocking(
            move || {
                pick_file(
                    parent,
                    tr!("discord.admin.upload-sound"),
                    &SOUND_TYPES,
                    SOUND_LIMIT,
                )
            },
            |chat: &mut Chat, picked: Result<Option<Picked>, String>| match picked {
                Ok(Some(picked)) => {
                    let name = picked.name.clone();
                    chat.space_admin_mut().begin_upload(
                        Upload::Sound {
                            picked,
                            volume: DEFAULT_VOLUME,
                        },
                        &name,
                        "",
                        "",
                    );
                }
                Ok(None) => {}
                Err(error_message) => chat.manage_failed(error_message),
            },
        );
    }

    fn submit_upload(&mut self) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let admin = self.space_admin();
        let Some(upload) = admin.upload.as_ref().cloned() else {
            return;
        };
        let name = admin.docs.upload_name.text().trim().to_owned();
        let tags = admin.docs.upload_tags.text().trim().to_owned();
        let description = admin.docs.upload_description.text().trim().to_owned();
        let emoji = typed_emoji(&tags);
        let reload: fn(&mut Chat) = match upload {
            Upload::Sticker(_) | Upload::EditSticker(_) => Chat::load_stickers,
            Upload::Sound { .. } | Upload::EditSound { .. } => Chat::load_sounds,
        };
        self.space_admin_mut().saving.set(true);
        let settled = move |chat: &mut Chat, saved: Option<()>| {
            chat.admin_saved(space);
            if saved.is_some() {
                chat.space_admin_mut().upload.set(None);
                reload(chat);
            }
        };
        match upload {
            Upload::Sticker(picked) => {
                let draft = StickerDraft {
                    name,
                    description,
                    tags,
                    file_name: format!("{}.{}", picked.name, extension(&picked.mime)),
                    mime: picked.mime,
                    bytes: picked.bytes,
                };
                self.attempt(
                    &key,
                    move |client| {
                        let draft = draft.clone();
                        async move { client.create_sticker(&space, draft).await.map(|_| ()) }
                    },
                    settled,
                );
            }
            Upload::EditSticker(sticker) => self.attempt(
                &key,
                move |client| {
                    let (name, description, tags) =
                        (name.clone(), description.clone(), tags.clone());
                    async move {
                        client
                            .edit_sticker(&space, &sticker, &name, &description, &tags)
                            .await
                    }
                },
                settled,
            ),
            Upload::Sound { picked, volume } => {
                let draft = SoundDraft {
                    name,
                    volume,
                    emoji,
                    mime: picked.mime,
                    bytes: picked.bytes,
                };
                self.attempt(
                    &key,
                    move |client| {
                        let draft = draft.clone();
                        async move { client.create_sound(&space, draft).await.map(|_| ()) }
                    },
                    settled,
                );
            }
            Upload::EditSound { id, volume } => self.attempt(
                &key,
                move |client| {
                    let (name, emoji) = (name.clone(), emoji.clone());
                    async move { client.edit_sound(&space, &id, &name, volume, emoji).await }
                },
                settled,
            ),
        }
    }

    fn ask_delete_sticker(&mut self, sticker: &StickerId) {
        let sticker = *sticker;
        self.confirm_delete(
            tr!("discord.admin.delete-sticker"),
            move |client, space| async move { client.delete_sticker(&space, &sticker).await },
            Chat::load_stickers,
        );
    }

    fn ask_delete_sound(&mut self, sound: &SoundId) {
        let sound = *sound;
        self.confirm_delete(
            tr!("discord.admin.delete-sound"),
            move |client, space| async move { client.delete_sound(&space, &sound).await },
            Chat::load_sounds,
        );
    }

    fn confirm_delete<F, Fut>(&mut self, title: Label, task: F, reload: fn(&mut Chat))
    where
        F: Fn(Client, SpaceId) -> Fut + Clone + 'static,
        Fut: Future<Output = discord_core::Result<()>> + Send + 'static,
    {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        self.confirm(
            Prompt {
                title: title.to_string(),
                body: tr!("discord.admin.delete-expression-body").to_string(),
                accept: tr!("discord.manage.delete").to_string(),
            },
            move |chat: &mut Chat| {
                let task = task.clone();
                chat.act(
                    &key,
                    move |client| task(client, space),
                    move |chat: &mut Chat, ()| reload(chat),
                );
            },
        );
    }
}

fn typed_emoji(text: &str) -> Option<Emoji> {
    match text.parse::<EmojiId>() {
        _ if text.is_empty() => None,
        Ok(id) => Some(Emoji {
            id: Some(id),
            name: String::new(),
            picture: None,
        }),
        Err(_) => Some(Emoji::unicode(text)),
    }
}

fn extension(mime: &str) -> &'static str {
    match mime {
        "image/gif" => "gif",
        "application/json" => "json",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        _ => "png",
    }
}
