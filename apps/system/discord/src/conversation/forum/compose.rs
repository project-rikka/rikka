use discord_core::{Abilities, Channel, ForumSettings};
use rikka_app::prelude::*;
use rikka_ui::{DragAction, DragOffer};
use rikka_widgets::{column, row};

use super::{EMOJI_BUTTON, Forum, tags};
use crate::chat::Chat;
use crate::conversation::composer::preview;
use crate::conversation::composer::uploads::{self, Attach};
use crate::conversation::overlay::{Overlay, PickerTarget, Tab};
use crate::widgets::with_tooltip;

const TITLE_CHARS: usize = 100;
const TITLE_SIZE: f32 = 20.0;
const BODY_SIZE: f32 = 15.0;
const LINE: f32 = 22.0;
const SHORTEST: f32 = 44.0;
const TALLEST: f32 = 320.0;
const ADD_MEDIA: f32 = 88.0;
const PREVIEW: f32 = 72.0;
const ICON: f32 = 20.0;

pub(super) fn view(forum: &Channel, settings: &ForumSettings, state: &Forum) -> Element {
    let attaching = forum.abilities.contains(Abilities::ATTACH);
    let manages = forum.abilities.contains(Abilities::MANAGE_MESSAGES);
    let ready = state.ready(settings);
    let posting = *state.posting;
    let head = row! {
        with_tooltip(
            "discord-post-close",
            button(icon(MaterialSymbol::CLOSE).extent(Length::px(ICON)))
                .icon_only()
                .ghost()
                .small()
                .focusless()
                .on_press(|forum: &mut Forum| forum.cancel()),
            tr!("discord.forum.close"),
        );
        column! {
            text_field(&state.title)
                .placeholder(tr!("discord.forum.title"))
                .plain(true)
                .font_size(TITLE_SIZE)
                .max_len(TITLE_CHARS)
                .autofocus(true)
                .on_change(|forum: &mut Forum| forum.refill())
                .fill_width();
            text_editor(&state.body)
                .placeholder(tr!("discord.forum.message"))
                .gutter(false)
                .font_size(BODY_SIZE)
                .line_height(LINE)
                .fits_text(SHORTEST..=TALLEST)
                .on_change(|forum: &mut Forum| forum.refill())
                .on_paste(|chat: &mut Chat, offered: Vec<String>| chat.paste_into(Attach::Post, offered))
                .fill_width();
        }
        .gap(Space::TIGHT)
        .grow(1.0)
        .min_width(0.0);
        if attaching {
            with_tooltip(
                "discord-post-media",
                container(
                    icon(MaterialSymbol::ADD_PHOTO_ALTERNATE)
                        .extent(Length::px(28.0))
                        .color(palette().text_muted),
                )
                .center()
                .background(palette().surface_variant)
                .radius(Corner::Control)
                .size(Length::px(ADD_MEDIA), Length::px(ADD_MEDIA))
                .focusless()
                .on_press(|chat: &mut Chat| chat.attach(Attach::Post)),
                tr!("discord.forum.add-media"),
            );
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Start)
    .fill_width();
    let wants_tag = settings.requires_tag && state.chosen.is_empty();
    let body = column! {
        head;
        if !state.uploads.is_empty() {
            strip(state);
        }
        if !settings.tags.is_empty() {
            row! {
                icon(MaterialSymbol::SELL).extent(Length::px(18.0)).color(palette().text_muted);
                tags::choice_bar(settings, &state.chosen, manages).grow(1.0).min_width(0.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
        }
        if wants_tag {
            text(tr!("discord.forum.tag-required")).caption().muted().fill_width();
        }
        if let Some(refused) = state.refused.as_deref() {
            text(refused.to_owned()).caption().danger().fill_width();
        }
        rule();
        row! {
            button(icon(MaterialSymbol::MOOD).extent(Length::px(22.0)))
                .icon_only()
                .ghost()
                .focusless()
                .on_press(|overlay: &mut Overlay| overlay.open_picker(PickerTarget::Post, Tab::Emoji))
                .build()
                .keyed(EMOJI_BUTTON);
            spacer().grow(1.0);
            button(
                row! {
                    icon(MaterialSymbol::CHAT_BUBBLE).extent(Length::px(18.0));
                    text(tr!("discord.forum.post"));
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center),
            )
            .primary()
            .enabled(ready)
            .loading(posting)
            .on_press(|chat: &mut Chat| chat.submit_post());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
    }
    .gap(Space::SNUG)
    .fill_width();
    let card = container(body)
        .padding(Inset::all(Space::NORMAL))
        .background(palette().surface)
        .border(1.0, palette().outline)
        .radius(Corner::Panel)
        .fill_width();
    drop_zone(card)
        .accepts(move |offer: &DragOffer| attaching.then(|| uploads::accepts(offer)).flatten())
        .on_drop(|chat: &mut Chat, offer: DragOffer, action: DragAction| {
            chat.drop_into(Attach::Post, offer, action)
        })
        .fill_width()
        .any()
}

fn strip(state: &Forum) -> Element {
    row_of(state.uploads.iter().map(|pending| {
        let id = pending.id;
        column! {
            preview(pending, PREVIEW);
            row! {
                text(pending.upload.name.clone()).caption().no_wrap().grow(1.0).min_width(0.0);
                button(icon(MaterialSymbol::CLOSE).extent(Length::px(14.0)))
                    .icon_only()
                    .ghost()
                    .small()
                    .focusless()
                    .on_press(move |forum: &mut Forum| forum.remove_upload(id));
            }
            .align_items(Align::Center)
            .width(PREVIEW);
        }
        .gap(Space::HAIR)
        .any()
    }))
    .gap(Space::SNUG)
    .wrap_items()
    .fill_width()
    .any()
}
