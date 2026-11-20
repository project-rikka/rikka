use discord_core::{MessageId, Reaction};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::row;

use crate::chat::Chat;
use crate::conversation::emoji::face;
use crate::conversation::overlay::{Overlay, PickerTarget, Tab};

pub(crate) fn view(message: &MessageId, reactions: &[Reaction], reacting: bool) -> Option<Element> {
    if reactions.is_empty() {
        return None;
    }
    let mut chips: Vec<Element> = reactions
        .iter()
        .map(|reaction| chip(message, reaction, reacting))
        .collect();
    if reacting {
        let id = *message;
        chips.push(
            container(
                icon(MaterialSymbol::ADD_REACTION)
                    .extent(Length::px(18.0))
                    .color(palette().text_muted),
            )
            .padding(Inset::xy(Space::SNUG, Space::TIGHT))
            .background(palette().surface)
            .radius(Corner::Item)
            .focusless()
            .on_press(move |overlay: &mut Overlay| {
                overlay.open_picker(PickerTarget::React(id), Tab::Emoji)
            })
            .any(),
        );
    }
    Some(
        row_of(chips)
            .gap(Space::TIGHT)
            .wrap_items()
            .fill_width()
            .any(),
    )
}

fn chip(message: &MessageId, reaction: &Reaction, reacting: bool) -> Element {
    let (fill, edge, ink) = match reaction.me {
        true => (
            palette().accent.with_alpha(0.16),
            palette().accent,
            palette().accent,
        ),
        false => (palette().surface, palette().outline, palette().text_muted),
    };
    let body = container(
        row! {
            face(&reaction.emoji, 18.0);
            text(reaction.count.to_string()).caption().semibold().color(ink).no_wrap();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .padding(Inset::xy(Space::SNUG, Space::HAIR))
    .background(fill)
    .border(1.0, edge)
    .radius(Corner::Item);
    if !reacting {
        return body.any();
    }
    let (id, emoji, on) = (*message, reaction.emoji.clone(), !reaction.me);
    body.focusless()
        .on_press(move |chat: &mut Chat| chat.react(&id, &emoji, on))
        .any()
}
