pub(crate) mod catalogue;

use std::rc::Rc;

use discord_core::{Abilities, Emoji, Sticker, StickerFormat};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::column;

use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::conversation::overlay::{Overlay, OverlayAnchor, PickerTarget, Tab};
use crate::pictures;
use rikka_emoji::Group;

const ACROSS: usize = 8;
const STICKERS_ACROSS: usize = 4;
const CELL: f32 = 38.0;
const STICKER: f32 = 76.0;
const WIDTH: f32 = 360.0;
const HEIGHT: f32 = 420.0;
const FOUND: usize = 64;
const RECENT: usize = 24;
const CUSTOM: &str = "custom:";
const RECENTS: &str = "recent-emoji.json";

#[derive(Clone)]
enum Line {
    Header(String),
    Group(Group),
    Glyphs(Vec<Emoji>),
    Stickers(Vec<Sticker>),
}

pub(crate) fn recent_key(emoji: &Emoji) -> String {
    match &emoji.id {
        Some(id) => format!("{CUSTOM}{id}"),
        None => emoji.name.clone(),
    }
}

pub(crate) fn resolve(linked: &LinkedAccount, key: &str) -> Option<Emoji> {
    match key.strip_prefix(CUSTOM) {
        Some(id) => linked
            .emoji_spaces()
            .into_iter()
            .flat_map(|(_, emojis)| emojis.iter())
            .find(|emoji| emoji.id.is_some_and(|known| known.to_string() == id))
            .cloned(),
        None => Some(Emoji::unicode(key)),
    }
}

pub(crate) fn recents_after(mut recents: Vec<String>, picked: &Emoji) -> Vec<String> {
    let key = recent_key(picked);
    recents.retain(|known| *known != key);
    recents.insert(0, key);
    recents.truncate(RECENT);
    recents
}

pub(crate) fn picker(
    chat: &Chat,
    picker_target: &PickerTarget,
    tab: Tab,
    anchor: &OverlayAnchor,
) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().any();
    };
    let overlay = chat.conversation().overlay();
    let channel = chat
        .conversation()
        .shown()
        .and_then(|(_, channel)| linked.channel(channel));
    let space = channel.and_then(|channel_info| channel_info.space);
    let foreign = channel
        .is_some_and(|channel_info| channel_info.abilities.contains(Abilities::EXTERNAL_EMOJI));
    let query = overlay.search().to_owned();
    let lines = match tab {
        Tab::Emoji => emoji_lines(linked, chat, &query, space.as_ref(), foreign),
        Tab::Stickers => sticker_lines(linked, &query),
    };
    let jumped = overlay.jumped().and_then(|group| {
        lines
            .iter()
            .position(|line| matches!(line, Line::Group(listed) if *listed == group))
    });
    let groups = (tab == Tab::Emoji && query.is_empty()).then(|| {
        row_of(Group::ALL.into_iter().map(|group| {
            container(
                icon(catalogue::symbol(group))
                    .extent(Length::px(18.0))
                    .color(palette().text_muted),
            )
            .center()
            .radius(Corner::Item)
            .size(Length::px(CELL), Length::px(CELL - 8.0))
            .focusless()
            .on_press(move |overlay: &mut Overlay| overlay.jump(group))
            .any()
        }))
        .gap(Space::HAIR)
        .fill_width()
    });
    let lines = Rc::new(lines);
    let shown = lines.clone();
    let body = match lines.is_empty() {
        true => status_page(
            MaterialSymbol::SEARCH,
            tr!("discord.conversation.nothing-found"),
        )
        .fill()
        .any(),
        false => lazy_column(lines.len(), {
            let picker_target = picker_target.clone();
            move |slot| line(&shown[slot.index], &picker_target)
        })
        .gap(Space::HAIR)
        .reveals(jumped)
        .grow(1.0)
        .fill_width()
        .any(),
    };
    let tabs = match picker_target {
        PickerTarget::Composer => Some(
            segmented(match tab {
                Tab::Emoji => 0,
                Tab::Stickers => 1,
            })
            .segment(tr!("discord.conversation.emoji-tab"))
            .segment(tr!("discord.conversation.stickers-tab"))
            .on_select(|overlay: &mut Overlay, picked: usize| {
                overlay.switch_tab(match picked {
                    0 => Tab::Emoji,
                    _ => Tab::Stickers,
                })
            })
            .fill_width(),
        ),
        PickerTarget::React(_) => None,
    };
    popover(
        column! {
            if let Some(tabs) = tabs {
                tabs;
            }
            text_field(overlay.query())
                .placeholder(tr!("discord.conversation.find-emoji"))
                .autofocus(true)
                .on_change(|overlay: &mut Overlay| overlay.searched())
                .fill_width();
            if let Some(groups) = groups {
                groups;
            }
            body;
        }
        .gap(Space::SNUG)
        .size(Length::px(WIDTH), Length::px(HEIGHT)),
    )
    .on_dismiss(|overlay: &mut Overlay| overlay.close())
    .build()
    .anchored(anchor.anchor())
}

fn emoji_lines(
    linked: &LinkedAccount,
    chat: &Chat,
    query: &str,
    space: Option<&discord_core::SpaceId>,
    foreign: bool,
) -> Vec<Line> {
    let usable: Vec<(String, Vec<Emoji>)> = linked
        .emoji_spaces()
        .into_iter()
        .filter(|(emoji_space, _)| foreign || space == Some(&emoji_space.id))
        .map(|(emoji_space, emojis)| (emoji_space.name.clone(), emojis.to_vec()))
        .collect();
    let mut lines = Vec::new();
    if !query.is_empty() {
        let mut matched: Vec<Emoji> = usable
            .iter()
            .flat_map(|(_, emojis)| emojis.iter())
            .filter(|emoji| emoji.name.to_lowercase().contains(query))
            .cloned()
            .collect();
        matched.extend(
            catalogue::search(query, FOUND)
                .into_iter()
                .map(|glyph| Emoji::unicode(glyph.text)),
        );
        push_rows(&mut lines, matched, ACROSS);
        return lines;
    }
    let recents: Vec<Emoji> = chat
        .conversation()
        .overlay()
        .recents()
        .iter()
        .filter_map(|key| resolve(linked, key))
        .collect();
    if !recents.is_empty() {
        lines.push(Line::Header(tr!("discord.conversation.recent").to_string()));
        push_rows(&mut lines, recents, ACROSS);
    }
    for (name, emojis) in usable {
        lines.push(Line::Header(name));
        push_rows(&mut lines, emojis, ACROSS);
    }
    for group in Group::ALL {
        lines.push(Line::Group(group));
        push_rows(
            &mut lines,
            catalogue::in_group(group)
                .into_iter()
                .map(Emoji::unicode)
                .collect(),
            ACROSS,
        );
    }
    lines
}

fn sticker_lines(linked: &LinkedAccount, query: &str) -> Vec<Line> {
    let mut lines = Vec::new();
    for (space, stickers) in linked.sticker_spaces() {
        let shown: Vec<Sticker> = stickers
            .iter()
            .filter(|sticker| query.is_empty() || sticker.name.to_lowercase().contains(query))
            .cloned()
            .collect();
        if shown.is_empty() {
            continue;
        }
        lines.push(Line::Header(space.name.clone()));
        for chunk in shown.chunks(STICKERS_ACROSS) {
            lines.push(Line::Stickers(chunk.to_vec()));
        }
    }
    lines
}

fn push_rows(lines: &mut Vec<Line>, emojis: Vec<Emoji>, across: usize) {
    for chunk in emojis.chunks(across) {
        lines.push(Line::Glyphs(chunk.to_vec()));
    }
}

fn header(name: String) -> Element {
    container(text(name).caption().semibold().muted().no_wrap())
        .padding(Inset::edges(
            Space::SNUG,
            Space::NONE,
            Space::HAIR,
            Space::TIGHT,
        ))
        .any()
}

fn line(line: &Line, picker_target: &PickerTarget) -> Element {
    match line {
        Line::Header(name) => header(name.clone()),
        Line::Group(group) => header(catalogue::label(*group).to_string()),
        Line::Glyphs(emojis) => row_of(emojis.iter().map(|emoji| {
            let picked = emoji.clone();
            let picker_target = picker_target.clone();
            container(face(emoji, 28.0))
                .center()
                .radius(Corner::Item)
                .size(Length::px(CELL), Length::px(CELL))
                .focusless()
                .on_press(move |chat: &mut Chat| chat.pick_emoji(&picker_target, &picked))
                .any()
        }))
        .gap(Space::HAIR)
        .any(),
        Line::Stickers(stickers) => row_of(stickers.iter().map(|sticker| {
            let picked = sticker.clone();
            container(sticker_face(sticker, STICKER))
                .center()
                .radius(Corner::Item)
                .size(Length::px(STICKER + 4.0), Length::px(STICKER + 4.0))
                .focusless()
                .on_press(move |chat: &mut Chat| chat.send_sticker(&picked))
                .any()
        }))
        .gap(Space::TIGHT)
        .any(),
    }
}

pub(crate) fn face(emoji: &Emoji, side: f32) -> Element {
    match &emoji.picture {
        Some(picture) => match pictures::fetched(picture, side) {
            Some(bytes) => image(bytes)
                .fit(ImageFit::Contain)
                .size(Length::px(side), Length::px(side))
                .any(),
            None => spacer().size(Length::px(side), Length::px(side)).any(),
        },
        None => text(emoji.name.clone())
            .text_size(Length::px(side * 0.85))
            .no_wrap()
            .any(),
    }
}

pub(crate) fn sticker_face(sticker: &Sticker, side: f32) -> Element {
    match (&sticker.picture, sticker.format) {
        (Some(picture), StickerFormat::Png | StickerFormat::Apng | StickerFormat::Gif) => {
            match pictures::fetched(picture, side) {
                Some(bytes) => image(bytes)
                    .fit(ImageFit::Contain)
                    .size(Length::px(side), Length::px(side))
                    .any(),
                None => spacer().size(Length::px(side), Length::px(side)).any(),
            }
        }
        _ => container(
            column! {
                icon(MaterialSymbol::STICKY_NOTE_2).extent(Length::px(side / 3.0)).color(palette().text_muted);
                text(sticker.name.clone()).caption().no_wrap();
            }
            .gap(Space::HAIR)
            .align_items(Align::Center),
        )
        .center()
        .background(palette().surface_variant)
        .radius(Corner::Item)
        .size(Length::px(side), Length::px(side))
        .any(),
    }
}

impl Chat {
    pub(crate) fn pick_emoji(&mut self, picker_target: &PickerTarget, emoji: &Emoji) {
        let recents = recents_after(self.conversation().overlay().recents().to_vec(), emoji);
        self.conversation_mut().overlay.remember(recents);
        self.save_recents();
        match picker_target {
            PickerTarget::Composer => self.insert_emoji(emoji),
            PickerTarget::React(message) => {
                self.conversation_mut().overlay.close();
                self.react(message, emoji, true);
            }
        }
    }

    pub(crate) fn recall_recents(&mut self, key: &str) {
        if self.conversation().overlay().recents_of() == Some(key) {
            return;
        }
        let path = self.accounts().folder(key).join(RECENTS);
        let key = key.to_owned();
        spawn_blocking_keyed(
            "discord-recent-emoji",
            move || rikka_account_book::read_json::<Vec<String>>(&path),
            move |chat: &mut Chat, loaded: rikka_account_book::Result<Option<Vec<String>>>| {
                if chat.current_key() != Some(key.as_str()) {
                    skip_view();
                    return;
                }
                let recents = loaded
                    .inspect_err(|error| tracing::debug!("failed to read recent emoji: {error}"))
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                chat.conversation_mut().overlay.recall(&key, recents);
            },
        );
    }

    fn save_recents(&mut self) {
        let overlay = self.conversation().overlay();
        let Some(key) = overlay.recents_of() else {
            return;
        };
        let path = self.accounts().folder(key).join(RECENTS);
        let recents = overlay.recents().to_vec();
        spawn_blocking(
            move || rikka_account_book::write_private_json(&path, &recents),
            |saved: rikka_account_book::Result<()>| {
                if let Err(error) = saved {
                    tracing::debug!("failed to save recent emoji: {error}");
                }
                skip_view();
            },
        );
    }
}
