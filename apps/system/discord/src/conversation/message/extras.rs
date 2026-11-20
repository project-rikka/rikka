use discord_core::{Message, Reply, SpaceId, Sticker, SystemEvent};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::row;

use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::conversation::emoji::sticker_face;
use crate::conversation::time;
use crate::widgets::avatar;

pub(crate) const GUTTER: f32 = 56.0;
const STICKER: f32 = 160.0;
const EXCERPT: usize = 120;

pub(crate) fn reply_line(
    linked: &LinkedAccount,
    space: Option<&SpaceId>,
    reply: &Reply,
) -> Element {
    let target = reply.message;
    let line = match &reply.quoted {
        Some(quoted) => {
            let name = linked.display_name(space, &quoted.author);
            let color = linked
                .color_of(space, &quoted.author.id)
                .map_or(palette().text, Color::hex);
            let mut excerpt: String = quoted
                .excerpt
                .plain_text()
                .replace('\n', " ")
                .chars()
                .take(EXCERPT)
                .collect();
            if excerpt.is_empty() && quoted.has_attachments {
                excerpt = tr!("discord.conversation.click-to-see").to_string();
            }
            row! {
                avatar(quoted.author.avatar.as_ref(), &name, 16.0);
                text(match quoted.mentions_author {
                    true => format!("@{name}"),
                    false => name.clone(),
                })
                .caption()
                .semibold()
                .color(color)
                .no_wrap();
                text(excerpt).caption().muted().no_wrap().grow(1.0).min_width(0.0);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .any()
        }
        None => text(tr!("discord.conversation.reply-gone"))
            .caption()
            .italic()
            .muted()
            .no_wrap()
            .any(),
    };
    row! {
        container(spacer())
            .border(2.0, palette().outline)
            .radius(CornerRadii::new(6.0, 0.0, 0.0, 0.0))
            .size(Length::px(GUTTER / 2.0), Length::px(10.0))
            .margin(Inset::edges(Space::SNUG, Space::TIGHT, Space::NONE, Space::from(GUTTER / 2.0 - 2.0)));
        container(line)
            .focusless()
            .on_press(move |chat: &mut Chat| chat.jump_to(&target))
            .grow(1.0)
            .min_width(0.0);
    }
    .align_items(Align::Center)
    .fill_width()
    .any()
}

pub(crate) fn system_line(
    linked: &LinkedAccount,
    space: Option<&SpaceId>,
    message: &Message,
) -> Option<Element> {
    let discord_core::MessageKind::System(event) = &message.kind else {
        return None;
    };
    let name = linked.display_name(space, &message.author);
    let name = name.as_str();
    let other = |user: &discord_core::UserId| {
        linked
            .name_of(space, user)
            .unwrap_or_else(|| user.to_string())
    };
    let (symbol, tint, caption) = match event {
        SystemEvent::Joined => (
            MaterialSymbol::ARROW_FORWARD,
            palette().success,
            tr!("discord.conversation.system.joined", name = name),
        ),
        SystemEvent::Boosted => (
            MaterialSymbol::ROCKET_LAUNCH,
            palette().accent,
            tr!("discord.conversation.system.boosted", name = name),
        ),
        SystemEvent::Pinned => (
            MaterialSymbol::PUSH_PIN,
            palette().text_muted,
            tr!("discord.conversation.system.pinned", name = name),
        ),
        SystemEvent::Call { ended: None } => (
            MaterialSymbol::CALL,
            palette().success,
            tr!("discord.conversation.system.call", name = name),
        ),
        SystemEvent::Call { ended: Some(ended) } => (
            MaterialSymbol::CALL_END,
            palette().text_muted,
            tr!(
                "discord.conversation.system.call-ended",
                name = name,
                minutes = (ended.saturating_sub(message.at) / 60_000).max(1)
            ),
        ),
        SystemEvent::Renamed(title) => (
            MaterialSymbol::EDIT,
            palette().text_muted,
            tr!(
                "discord.conversation.system.renamed",
                name = name,
                title = title.as_str()
            ),
        ),
        SystemEvent::RecipientAdded(user) => (
            MaterialSymbol::PERSON_ADD,
            palette().success,
            tr!(
                "discord.conversation.system.added",
                name = name,
                other = other(user).as_str()
            ),
        ),
        SystemEvent::RecipientRemoved(user) => (
            MaterialSymbol::PERSON_REMOVE,
            palette().danger,
            tr!(
                "discord.conversation.system.removed",
                name = name,
                other = other(user).as_str()
            ),
        ),
        SystemEvent::ThreadCreated(title) => (
            MaterialSymbol::FORUM,
            palette().text_muted,
            tr!(
                "discord.conversation.system.thread",
                name = name,
                title = title.as_str()
            ),
        ),
        SystemEvent::Other(caption) => (
            MaterialSymbol::INFO,
            palette().text_muted,
            Label::from(caption.clone()),
        ),
    };
    Some(
        row! {
            container(icon(symbol).extent(Length::px(18.0)).color(tint))
                .center()
                .width(GUTTER);
            text(caption).body().muted().min_width(0.0);
            text(time::sent_at(message.at)).caption().muted().no_wrap();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::edges(
            Space::SNUG,
            Space::ROOMY,
            Space::TIGHT,
            Space::ROOMY,
        ))
        .fill_width()
        .any(),
    )
}

pub(crate) fn stickers(stickers: &[Sticker]) -> Option<Element> {
    (!stickers.is_empty()).then(|| {
        row_of(
            stickers
                .iter()
                .map(|sticker| sticker_face(sticker, STICKER)),
        )
        .gap(Space::SNUG)
        .any()
    })
}

pub(crate) fn thread_chip(linked: &LinkedAccount, message: &Message) -> Option<Element> {
    let id = message.thread?;
    let thread = linked.channel(&id)?;
    let count = thread.thread.as_ref().map_or(0, |about| about.messages);
    let channel_id = id;
    Some(
        container(
            row! {
                icon(MaterialSymbol::FORUM).extent(Length::px(16.0)).color(palette().text_muted);
                text(thread.name.clone()).label().semibold().no_wrap().min_width(0.0);
                text(tr!("discord.conversation.messages", count = i64::from(count)))
                    .caption()
                    .accent()
                    .no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .padding(Inset::xy(Space::SNUG, Space::TIGHT))
        .background(palette().surface)
        .border(1.0, palette().outline)
        .radius(Corner::Item)
        .focusless()
        .on_press(move |chat: &mut Chat| chat.open_channel(&channel_id))
        .max_width(420.0)
        .any(),
    )
}
