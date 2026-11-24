use discord_core::ChannelKind;
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::column;

use super::Overlay;
use crate::chat::Chat;
use crate::conversation::header::TOPIC;
use crate::conversation::rich_view;
use crate::widgets::mark_of;

const WIDTH: f32 = 440.0;
const TALLEST: f32 = 480.0;

pub(super) fn view(chat: &Chat) -> Option<Element> {
    let (_, channel) = chat.conversation().shown()?;
    let linked = chat.current()?;
    let channel_info = linked.channel(channel)?;
    let topic = channel_info.topic.as_ref()?;
    let (heading, edge) = match channel_info.kind {
        ChannelKind::Forum => (tr!("discord.forum.guidelines"), AnchorEdge::BelowEnd),
        _ => (
            Label::from(linked.channel_title(channel_info)),
            AnchorEdge::BelowStart,
        ),
    };
    let body = rich_view(
        linked,
        channel_info.space.as_ref(),
        topic,
        mark_of(("discord-topic", channel_info.id)),
        chat.appearance().animated_emoji,
    );
    Some(
        popover(
            column! {
                text(heading).heading().semibold().fill_width();
                scrollable(body).max_height(TALLEST).fill_width();
            }
            .gap(Space::NORMAL)
            .width(WIDTH),
        )
        .on_dismiss(|overlay: &mut Overlay| overlay.close())
        .build()
        .anchored(Anchor::new(TOPIC, edge).offset(0.0, Space::TIGHT.px())),
    )
}
