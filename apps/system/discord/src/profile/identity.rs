use discord_core::Badge;
use rikka_app::prelude::*;
use rikka_ui::Handler;
use rikka_widgets::{column, row};

use super::look::Look;
use crate::conversation::open_link;
use crate::pictures;
use crate::widgets::with_tooltip;

pub(super) fn identity(look: &Look<'_>, size: f32, on_open: Option<Handler<()>>) -> Element {
    let mut name = container(
        text(look.name.clone())
            .text_size(Length::px(size))
            .bold()
            .no_wrap(),
    )
    .focusless()
    .min_width(0.0);
    if let Some(on_open) = on_open {
        name = name.on_press(on_open);
    }
    let mut handle: Vec<Element> = vec![text(look.user.handle.clone()).label().no_wrap().any()];
    if let Some(pronouns) = look.pronouns() {
        handle.push(text("•").label().muted().no_wrap().any());
        handle.push(text(pronouns.to_owned()).label().muted().no_wrap().any());
    }
    let badges = badges(look, (size * 1.1).round());
    column! {
        row! {
            name;
            if look.user.bot {
                container(text(tr!("discord.profile.bot")).caption().bold().color(palette().on_accent))
                    .padding(Inset::xy(Space::TIGHT, Space::NONE))
                    .background(palette().accent)
                    .radius(Corner::Detail);
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        row! {
            row_of(handle).gap(Space::TIGHT).align_items(Align::Center).min_width(0.0);
            if !badges.is_empty() {
                row_of(badges).gap(Space::HAIR).align_items(Align::Center);
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .wrap_items()
        .fill_width();
    }
    .gap(Space::HAIR)
    .fill_width()
    .any()
}

fn badges(look: &Look<'_>, side: f32) -> Vec<Element> {
    let Some(profile) = look.profile.as_ref() else {
        return Vec::new();
    };
    profile
        .badges
        .iter()
        .map(|badge| shown_badge(look, badge, side))
        .collect()
}

fn shown_badge(look: &Look<'_>, badge: &Badge, side: f32) -> Element {
    let face = match pictures::fetched(&badge.icon, side) {
        Some(bytes) => image(bytes).fit(ImageFit::Contain).any(),
        None => spacer().any(),
    };
    let mut body = container(face)
        .focusless()
        .size(Length::px(side), Length::px(side));
    if let Some(link) = badge.link.clone() {
        body = body.on_press(move || {
            open_link(&link);
            skip_view();
        });
    }
    with_tooltip(
        ("discord-badge", look.mark(), &badge.id),
        body,
        badge.description.clone(),
    )
}
