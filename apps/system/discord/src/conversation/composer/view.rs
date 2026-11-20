use std::sync::Arc;

use discord_core::{Abilities, Channel, ChannelId, ChannelKind, UploadSource};
use rikka_app::prelude::*;
use rikka_ui::{DragAction, DragOffer, Element, ImageSource};
use rikka_widgets::{column, row};

use super::complete::Look;
use super::uploads::{self, Attach, Pending};
use super::{Composer, typing};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::conversation::overlay::{Overlay, PickerTarget, Tab};
use crate::pictures;
use crate::widgets::avatar;

pub(crate) const FIELD: &str = "discord-composer-field";
pub(crate) const EMOJI_BUTTON: &str = "discord-composer-emoji";
const SHORTEST: f32 = 22.0;
const TALLEST: f32 = 264.0;
const LINE: f32 = 22.0;
const WARN_WITHIN: usize = 200;
const TILE: f32 = 132.0;
const PREVIEW: f32 = 88.0;
const OFFERS: f32 = 360.0;

pub(crate) fn sendable(channel: &Channel) -> bool {
    match channel.kind {
        ChannelKind::Thread => {
            let open = channel.thread.as_ref().is_none_or(|thread| {
                !thread.locked || channel.abilities.contains(Abilities::MANAGE_MESSAGES)
            });
            open && (channel.abilities.contains(Abilities::SEND_IN_THREADS)
                || channel.abilities.contains(Abilities::SEND))
        }
        _ => channel.abilities.contains(Abilities::SEND),
    }
}

pub(crate) fn view(chat: &Chat, channel: &ChannelId) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().any();
    };
    let Some(channel_info) = linked.channel(channel) else {
        return spacer().any();
    };
    let composer = chat.conversation().composing();
    let foot = foot(linked, channel_info, composer);
    let body = match sendable(channel_info) {
        false => container(
            text(tr!("discord.conversation.read-only"))
                .label()
                .muted()
                .align(TextAlign::Center)
                .fill_width(),
        )
        .padding(Inset::all(Space::NORMAL))
        .background(palette().surface)
        .radius(Corner::Control)
        .fill_width()
        .any(),
        true => writing(linked, channel_info, composer),
    };
    let mut parts = vec![body, foot];
    if let Some(offers) = offered(composer) {
        parts.push(offers);
    }
    column_of(parts)
        .gap(Space::TIGHT)
        .padding(Inset::edges(
            Space::NONE,
            Space::ROOMY,
            Space::SNUG,
            Space::ROOMY,
        ))
        .fill_width()
        .any()
}

fn writing(linked: &LinkedAccount, channel: &Channel, composer: &Composer) -> Element {
    let title = linked.channel_title(channel);
    let placeholder = match channel.kind {
        ChannelKind::Direct | ChannelKind::Group => {
            tr!("discord.conversation.message-person", name = title.as_str())
        }
        _ => tr!(
            "discord.conversation.message-channel",
            name = title.as_str()
        ),
    };
    let attaching = channel.abilities.contains(Abilities::ATTACH);
    let target = channel.id;
    let editor = text_editor(composer.draft())
        .placeholder(placeholder)
        .gutter(false)
        .font_size(15.0)
        .line_height(LINE)
        .fits_text(SHORTEST..=TALLEST)
        .autofocus(composer.editing().is_none())
        .on_change(|chat: &mut Chat| chat.composer_changed())
        .on_submit(move |chat: &mut Chat| chat.send_draft(&target))
        .on_paste(|chat: &mut Chat, offered: Vec<String>| chat.paste_into(Attach::Message, offered))
        .on_key(|chat: &mut Chat, key: Key, modifiers: Modifiers| chat.composer_key(key, modifiers))
        .build()
        .keyed(("discord-draft", linked.key().to_owned(), channel.id))
        .grow(1.0)
        .min_width(0.0);
    let line = row! {
        if attaching {
            button(icon(MaterialSymbol::ADD_CIRCLE).extent(Length::px(22.0)))
                .icon_only()
                .ghost()
                .focusless()
                .on_press(|chat: &mut Chat| chat.attach(Attach::Message));
        }
        editor;
        if channel.abilities.contains(Abilities::STICKERS) {
            button(icon(MaterialSymbol::STICKY_NOTE_2).extent(Length::px(22.0)))
                .icon_only()
                .ghost()
                .focusless()
                .on_press(|overlay: &mut Overlay| overlay.open_picker(PickerTarget::Composer, Tab::Stickers));
        }
        button(icon(MaterialSymbol::MOOD).extent(Length::px(22.0)))
            .icon_only()
            .ghost()
            .focusless()
            .on_press(|overlay: &mut Overlay| overlay.open_picker(PickerTarget::Composer, Tab::Emoji))
            .build()
            .keyed(EMOJI_BUTTON);
    }
    .gap(Space::TIGHT)
    .align_items(Align::End)
    .fill_width();
    let field = column! {
        if let Some(reply) = composer.reply() {
            reply_bar(&reply.author, reply.mention);
        }
        if !composer.uploads().is_empty() {
            strip(composer);
        }
        line;
    }
    .gap(Space::SNUG)
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .fill_width();
    let field = container(field)
        .background(palette().surface)
        .radius(Corner::Control)
        .fill_width()
        .build()
        .keyed(FIELD);
    let mut parts = Vec::new();
    if let Some(refused) = composer.refused() {
        parts.push(
            banner(refused.to_owned())
                .on_dismiss(
                    tr!("discord.conversation.dismiss"),
                    |composer: &mut Composer| composer.dismiss_refusal(),
                )
                .fill_width()
                .any(),
        );
    }
    parts.push(
        drop_zone(field)
            .accepts(move |offer: &DragOffer| attaching.then(|| uploads::accepts(offer)).flatten())
            .on_drop(|chat: &mut Chat, offer: DragOffer, action: DragAction| {
                chat.drop_into(Attach::Message, offer, action)
            })
            .fill_width()
            .any(),
    );
    column_of(parts).gap(Space::TIGHT).fill_width().any()
}

fn reply_bar(author: &str, mention: bool) -> Element {
    let ping = match mention {
        true => tr!("discord.conversation.mention-on"),
        false => tr!("discord.conversation.mention-off"),
    };
    container(
        row! {
            text(tr!("discord.conversation.replying-to", name = author))
                .caption()
                .no_wrap()
                .grow(1.0)
                .min_width(0.0);
            button(text(ping).caption().semibold().color(match mention {
                true => palette().accent,
                false => palette().text_muted,
            }))
            .ghost()
            .small()
            .focusless()
            .on_press(|composer: &mut Composer| composer.toggle_reply_mention());
            button(icon(MaterialSymbol::CLOSE).extent(Length::px(16.0)))
                .icon_only()
                .ghost()
                .small()
                .focusless()
                .on_press(|composer: &mut Composer| composer.cancel_reply());
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::SNUG, Space::HAIR))
    .background(palette().surface_variant)
    .radius(Corner::Item)
    .fill_width()
    .any()
}

fn strip(composer: &Composer) -> Element {
    row_of(
        composer
            .uploads()
            .iter()
            .map(|pending| tile(composer, pending)),
    )
    .gap(Space::SNUG)
    .wrap_items()
    .fill_width()
    .any()
}

pub(crate) fn preview(pending: &Pending, side: f32) -> Element {
    let shown = match (&pending.upload.source, pending.is_picture()) {
        (UploadSource::Path(path), true) => Some(ImageSource::Path(Arc::from(path.as_path()))),
        (UploadSource::Bytes(bytes), true) => Some(ImageSource::Bytes(bytes.clone())),
        _ => None,
    };
    let face = match shown {
        Some(source) => image(source)
            .fit(ImageFit::Cover)
            .radius(Corner::Item)
            .size(Length::px(side), Length::px(side))
            .any(),
        None => container(
            icon(MaterialSymbol::DESCRIPTION)
                .extent(Length::px(side / 2.5))
                .color(palette().text_muted),
        )
        .center()
        .background(palette().surface_variant)
        .radius(Corner::Item)
        .size(Length::px(side), Length::px(side))
        .any(),
    };
    match pending.upload.spoiler {
        true => column_of([
            face.opacity(0.25).any(),
            container(text(tr!("discord.conversation.spoiler")).caption().bold())
                .center()
                .size(Length::px(side), Length::px(side))
                .position(0.0, 0.0)
                .any(),
        ])
        .size(Length::px(side), Length::px(side))
        .any(),
        false => face,
    }
}

fn tile(composer: &Composer, pending: &Pending) -> Element {
    let id = pending.id;
    let described = composer.descriptions.get(&id).cloned();
    column! {
        row! {
            preview(pending, PREVIEW);
            column! {
                button(icon(match pending.upload.spoiler {
                    true => MaterialSymbol::VISIBILITY,
                    false => MaterialSymbol::VISIBILITY_OFF,
                }).extent(Length::px(16.0)))
                .icon_only()
                .ghost()
                .small()
                .focusless()
                .on_press(move |composer: &mut Composer| composer.toggle_spoiler(id));
                button(icon(MaterialSymbol::DELETE).extent(Length::px(16.0)).color(palette().danger))
                    .icon_only()
                    .ghost()
                    .small()
                    .focusless()
                    .on_press(move |composer: &mut Composer| composer.remove_upload(id));
            }
            .gap(Space::HAIR);
        }
        .gap(Space::TIGHT);
        text(pending.upload.name.clone()).caption().no_wrap().fill_width();
        text(rikka_format::format_bytes(pending.upload.size)).caption().muted().no_wrap();
        if let Some(description) = described {
            text_field(&description)
                .placeholder(tr!("discord.conversation.description"))
                .control_size(ControlSize::Small)
                .fill_width();
        }
    }
    .gap(Space::HAIR)
    .padding(Inset::all(Space::SNUG))
    .background(palette().surface_variant.with_alpha(0.6))
    .width(TILE)
    .any()
}

fn foot(linked: &LinkedAccount, channel: &Channel, composer: &Composer) -> Element {
    let limit = linked.client().limits().message_chars;
    let draft = composer.draft().clone();
    let cooldown = composer.cooldown();
    let slowmode = channel.slowmode;
    row! {
        match typing::line(linked, &channel.id) {
            Some(line) => line.grow(1.0).min_width(0.0).any(),
            None => spacer().grow(1.0).any(),
        };
        if slowmode > 0 {
            row! {
                icon(MaterialSymbol::TIMER).extent(Length::px(14.0)).color(palette().text_muted);
                text(match cooldown {
                    0 => tr!("discord.conversation.slowmode", seconds = i64::from(slowmode)),
                    left => tr!("discord.conversation.cooldown", seconds = i64::from(left)),
                })
                .caption()
                .muted()
                .no_wrap();
            }
            .gap(Space::HAIR)
            .align_items(Align::Center);
        }
        live_observed(move || {
            let used = draft.char_count();
            match used + WARN_WITHIN > limit {
                true => {
                    let left = limit as i64 - used as i64;
                    text(left.to_string())
                        .caption()
                        .semibold()
                        .color(match left < 0 {
                            true => palette().danger,
                            false => palette().text_muted,
                        })
                        .any()
                }
                false => spacer().width(0.0).any(),
            }
        });
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .height(20.0)
    .fill_width()
    .any()
}

fn offered(composer: &Composer) -> Option<Element> {
    let offers = composer.offers()?;
    let rows: Vec<Element> = offers
        .rows
        .iter()
        .enumerate()
        .map(|(index, offer)| {
            let face = match &offer.look {
                Look::Person(picture) => avatar(picture.as_ref(), &offer.label, 24.0),
                Look::Role(color) => icon(MaterialSymbol::ALTERNATE_EMAIL)
                    .extent(Length::px(20.0))
                    .color(color.map_or(palette().text_muted, Color::hex))
                    .any(),
                Look::Everyone => icon(MaterialSymbol::GROUP)
                    .extent(Length::px(20.0))
                    .color(palette().text_muted)
                    .any(),
                Look::Channel(kind) => icon(crate::conversation::header::symbol_of(*kind))
                    .extent(Length::px(20.0))
                    .color(palette().text_muted)
                    .any(),
                Look::Glyph(glyph) => text(*glyph).text_size(Length::px(20.0)).no_wrap().any(),
                Look::Custom(picture) => match pictures::fetched(picture, 24.0) {
                    Some(bytes) => image(bytes)
                        .fit(ImageFit::Contain)
                        .size(Length::px(24.0), Length::px(24.0))
                        .any(),
                    None => spacer().size(Length::px(24.0), Length::px(24.0)).any(),
                },
            };
            let lit = index == offers.picked;
            container(
                row! {
                    face;
                    text(offer.label.clone()).label().medium().no_wrap().min_width(0.0);
                    spacer().grow(1.0);
                    if let Some(detail) = &offer.detail {
                        text(detail.clone()).caption().muted().no_wrap();
                    }
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .fill_width(),
            )
            .padding(Inset::xy(Space::SNUG, Space::TIGHT))
            .background(match lit {
                true => palette().accent.with_alpha(0.18),
                false => Color::TRANSPARENT,
            })
            .radius(Corner::Item)
            .focusless()
            .on_press(move |chat: &mut Chat| chat.take_offer(index))
            .fill_width()
            .any()
        })
        .collect();
    Some(
        container(column_of(rows).gap(Space::HAIR).fill_width())
            .padding(Inset::all(Space::TIGHT))
            .background(palette().surface)
            .border(1.0, palette().outline)
            .radius(Corner::Control)
            .width(OFFERS)
            .build()
            .anchored(Anchor::above(FIELD).offset(0.0, -Space::TIGHT.px())),
    )
}
