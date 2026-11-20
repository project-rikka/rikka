pub(crate) mod items;
mod reading;

use std::ops::Range;
use std::rc::Rc;

use discord_core::{ChannelId, ChannelKind, MessageId};
use rikka_app::prelude::*;
use rikka_ui::{Element, Handler, hash_of};
use rikka_widgets::{column, row};

use crate::chat::Chat;
use crate::conversation::header::symbol_of;
use crate::conversation::media::notice_bar;
use crate::conversation::message::{self, Props};
use crate::conversation::overlay::{MARK, Overlay};
use crate::conversation::widgets::context_area;
use crate::conversation::{Conversation, time};
use items::{Item, items};
pub(crate) use reading::{JumpTarget, Reading, Viewport};

const ESTIMATE: f32 = 56.0;

pub(crate) fn view(chat: &Chat, channel: &ChannelId) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().fill().any();
    };
    let Some(channel_info) = linked.channel(channel) else {
        return spacer().fill().any();
    };
    let conversation = chat.conversation();
    let Some(timeline) = linked.timeline(channel) else {
        return match conversation.error_message() {
            Some(error_message) => status_page(MaterialSymbol::ERROR, error_message.to_owned())
                .tint(palette().danger)
                .action(
                    button(text(tr!("discord.conversation.dismiss")))
                        .ghost()
                        .on_press(|conversation: &mut Conversation| conversation.dismiss()),
                )
                .fill()
                .any(),
            None => container(spinner()).center().fill().any(),
        };
    };
    let reading = conversation.reading();
    let listed = items(
        timeline.messages(),
        reading.since(),
        timeline.reached_oldest(),
    );
    let count = listed.len();
    let revealing = reading.revealing().and_then(|message_id| {
        listed.iter().position(|item| {
            item.message()
                .is_some_and(|message| &message.id == message_id)
        })
    });
    let names: Rc<[u64]> = listed.iter().map(Item::name).collect();
    let subject = hash_of((linked.key(), channel));
    let fresh = reading.left_at().map_or(0, |left| {
        timeline
            .messages()
            .iter()
            .rev()
            .take_while(|message| &message.id != left)
            .count()
    });
    let viewport = Viewport {
        key: linked.key().to_owned(),
        channel: *channel,
        count,
        revealing,
        last: timeline.messages().last().map(|message| message.id),
        oldest: timeline.reached_oldest(),
        newest: timeline.reached_newest(),
    };
    let title = linked.channel_title(channel_info);
    let opening = Opening {
        kind: channel_info.kind,
        title,
    };
    let listed = Rc::new(listed);
    let cells = Cells {
        key: linked.key().to_owned(),
        channel: *channel,
        opening,
        editing: conversation.composing().editing().cloned(),
        flashed: reading.flashed().cloned(),
    };
    let keys = Rc::clone(&names);
    let list = lazy_column(count, move |slot| {
        cells
            .cell(&listed[slot.index])
            .keyed((subject, keys[slot.index]))
    })
    .estimate(ESTIMATE)
    .padding(Inset::edges(
        Space::NONE,
        Space::NONE,
        Space::ROOMY,
        Space::NONE,
    ))
    .follow(timeline.reached_newest() && revealing.is_none())
    .shows(subject)
    .named(move |index| names.get(index).copied().unwrap_or_default())
    .reveals(revealing)
    .place(reading.place())
    .on_visible(move |chat: &mut Chat, shown: Range<usize>| chat.visible_rows(&viewport, shown))
    .fill_width()
    .grow(1.0);
    let open = Handler::new(|overlay: &mut Overlay, at: Point| overlay.open_menu_at(at));
    column! {
        if let Some(error_message) = conversation.error_message() {
            banner(error_message.to_owned())
                .on_dismiss(tr!("discord.conversation.dismiss"), |conversation: &mut Conversation| {
                    conversation.dismiss()
                })
                .fill_width();
        }
        if let Some(notice) = conversation.media().notice() {
            notice_bar(notice);
        }
        jump_to_end(
            row! {
                spacer().width(0.0).fill_height().build().keyed(MARK);
                context_area(list, open).grow(1.0).fill_height();
            }
            .grow(1.0)
            .fill_width(),
            reading.place(),
        )
        .unseen(fresh)
        .more_beyond(!timeline.reached_newest())
        .on_press(|chat: &mut Chat| chat.jump_to_present())
        .grow(1.0)
        .fill_width();
    }
    .fill()
    .any()
}

#[derive(Clone)]
struct Opening {
    kind: ChannelKind,
    title: String,
}

struct Cells {
    key: String,
    channel: ChannelId,
    opening: Opening,
    editing: Option<MessageId>,
    flashed: Option<MessageId>,
}

impl Cells {
    fn cell(&self, item: &Item) -> Element {
        match item {
            Item::Opening => welcome(&self.opening),
            Item::Older => container(spinner())
                .center()
                .height(48.0)
                .fill_width()
                .any(),
            Item::Day(at) => separator(&time::day_label(*at)),
            Item::Unread => unread_divider(),
            Item::Message { message, head } => component_with(
                |chat: &Chat| chat,
                Props {
                    key: self.key.clone(),
                    channel: self.channel,
                    message: message.clone(),
                    head: *head,
                    editing: self.editing.as_ref() == Some(&message.id),
                    flashed: self.flashed.as_ref() == Some(&message.id),
                },
                message::view,
            ),
        }
    }
}

fn welcome(opening: &Opening) -> Element {
    let (headline, caption) = match opening.kind {
        ChannelKind::Direct | ChannelKind::Group => (
            opening.title.clone(),
            tr!(
                "discord.conversation.direct-start",
                name = opening.title.as_str()
            ),
        ),
        _ => (
            tr!(
                "discord.conversation.welcome",
                name = opening.title.as_str()
            )
            .to_string(),
            tr!(
                "discord.conversation.channel-start",
                name = opening.title.as_str()
            ),
        ),
    };
    column! {
        container(icon(symbol_of(opening.kind)).extent(Length::px(40.0)).color(palette().text))
            .center()
            .background(palette().surface_variant)
            .radius(34.0)
            .size(Length::px(68.0), Length::px(68.0));
        text(headline).title().bold();
        text(caption).body().muted();
    }
    .gap(Space::SNUG)
    .padding(Inset::edges(
        Space::WIDE,
        Space::ROOMY,
        Space::ROOMY,
        Space::ROOMY,
    ))
    .fill_width()
    .any()
}

fn separator(label: &str) -> Element {
    row! {
        rule().width(Length::HUG).grow(1.0);
        text(label.to_owned()).caption().semibold().muted().no_wrap();
        rule().width(Length::HUG).grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .padding(Inset::edges(
        Space::ROOMY,
        Space::ROOMY,
        Space::TIGHT,
        Space::ROOMY,
    ))
    .fill_width()
    .any()
}

fn unread_divider() -> Element {
    row! {
        container(spacer()).background(palette().danger).height(1.0).grow(1.0);
        container(text(tr!("discord.conversation.new")).caption().bold().color(palette().on_danger))
            .padding(Inset::xy(Space::TIGHT, Space::NONE))
            .background(palette().danger)
            .radius(Corner::Detail);
    }
    .align_items(Align::Center)
    .padding(Inset::edges(
        Space::SNUG,
        Space::ROOMY,
        Space::NONE,
        Space::ROOMY,
    ))
    .fill_width()
    .any()
}
