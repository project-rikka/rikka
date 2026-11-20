use std::sync::Arc;

use discord_core::{ChannelId, Message};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use super::{Listing, Overlay, Shown};
use crate::chat::Chat;
use crate::conversation::header::PINS;
use crate::conversation::time;
use crate::widgets::avatar;

const WIDTH: f32 = 420.0;
const HEIGHT: f32 = 480.0;
const EXCERPT: usize = 280;

impl Chat {
    pub(crate) fn show_pins(&mut self) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        let overlay = &mut self.conversation_mut().overlay;
        if matches!(*overlay.shown, Some(Shown::Pins)) {
            overlay.close();
            return;
        }
        overlay.pins.set(Listing::Loading);
        overlay.shown.set(Some(Shown::Pins));
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        let channel_id = channel;
        spawn_keyed(
            ("discord-pins", key.clone(), channel),
            async move { client.pinned(&channel_id).await },
            move |chat: &mut Chat, pinned: discord_core::Result<Vec<Message>>| {
                chat.on_pins_loaded(&key, &channel, pinned);
            },
        );
    }

    pub(crate) fn on_pins_loaded(
        &mut self,
        key: &str,
        channel: &ChannelId,
        pinned: discord_core::Result<Vec<Message>>,
    ) {
        if self
            .conversation()
            .shown()
            .is_none_or(|(shown, at)| shown != key || at != channel)
        {
            return;
        }
        let listing = match pinned {
            Ok(messages) => Listing::Ready(messages.into_iter().map(Arc::new).collect()),
            Err(error) => Listing::Failed(error.to_string()),
        };
        self.conversation_mut().overlay.pins.set(listing);
    }
}

pub(super) fn view(chat: &Chat) -> Element {
    let overlay = chat.conversation().overlay();
    let space = chat.conversation().shown().and_then(|(key, channel)| {
        chat.linked(key)
            .and_then(|linked| linked.channel(channel))
            .and_then(|channel_info| channel_info.space)
    });
    let body = match overlay.pins() {
        Listing::Loading => container(spinner()).center().fill().any(),
        Listing::Failed(caption) => status_page(MaterialSymbol::ERROR, caption.clone())
            .tint(palette().danger)
            .fill()
            .any(),
        Listing::Ready(pinned) if pinned.is_empty() => status_page(
            MaterialSymbol::PUSH_PIN,
            tr!("discord.conversation.no-pins"),
        )
        .fill()
        .any(),
        Listing::Ready(pinned) => {
            let rows: Vec<Element> = pinned
                .iter()
                .map(|message| row_of_pin(chat, space.as_ref(), message))
                .collect();
            scrollable(column_of(rows).gap(Space::SNUG).fill_width())
                .fill()
                .any()
        }
    };
    popover(
        column! {
            text(tr!("discord.conversation.pinned")).heading().semibold();
            body;
        }
        .gap(Space::NORMAL)
        .size(Length::px(WIDTH), Length::px(HEIGHT)),
    )
    .on_dismiss(|overlay: &mut Overlay| overlay.close())
    .build()
    .anchored(Anchor::new(PINS, AnchorEdge::BelowEnd).offset(0.0, Space::TIGHT.px()))
}

fn row_of_pin(
    chat: &Chat,
    space: Option<&discord_core::SpaceId>,
    message: &Arc<Message>,
) -> Element {
    let name = chat
        .current()
        .map(|linked| linked.display_name(space, &message.author))
        .unwrap_or_else(|| message.author.name().to_owned());
    let excerpt: String = message.content.plain_text().chars().take(EXCERPT).collect();
    let id = message.id;
    container(
        row! {
            avatar(message.author.avatar.as_ref(), &name, 32.0);
            column! {
                row! {
                    text(name.clone()).label().semibold().no_wrap().min_width(0.0);
                    text(time::sent_at(message.at)).caption().muted().no_wrap();
                    spacer().grow(1.0);
                    button(text(tr!("discord.conversation.jump")).caption())
                        .small()
                        .secondary()
                        .on_press(move |chat: &mut Chat| chat.jump_from_overlay(&id));
                }
                .gap(Space::SNUG)
                .align_items(Align::Center);
                text(excerpt).body().fill_width();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Start)
        .fill_width(),
    )
    .padding(Inset::all(Space::SNUG))
    .background(palette().surface_variant.with_alpha(0.5))
    .radius(Corner::Item)
    .fill_width()
    .any()
}
