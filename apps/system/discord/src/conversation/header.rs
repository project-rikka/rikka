use discord_core::{Channel, ChannelId, ChannelKind, Rich, Update};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::row;

use crate::chat::Chat;
use crate::conversation::Conversation;
use crate::conversation::overlay::{Overlay, Shown};

const HEIGHT: f32 = 48.0;
pub(crate) const PINS: &str = "discord-header-pins";
pub(crate) const THREADS: &str = "discord-header-threads";
pub(crate) const TOPIC: &str = "discord-header-topic";

pub(crate) fn symbol_of(kind: ChannelKind) -> MaterialSymbol {
    match kind {
        ChannelKind::Direct => MaterialSymbol::ALTERNATE_EMAIL,
        ChannelKind::Group => MaterialSymbol::GROUP,
        ChannelKind::Announcement => MaterialSymbol::CAMPAIGN,
        ChannelKind::Voice => MaterialSymbol::VOLUME_UP,
        ChannelKind::Stage => MaterialSymbol::PODCASTS,
        ChannelKind::Forum => MaterialSymbol::FORUM,
        ChannelKind::Thread => MaterialSymbol::SUBDIRECTORY_ARROW_RIGHT,
        ChannelKind::Text | ChannelKind::Category => MaterialSymbol::TAG,
    }
}

fn tool(symbol: MaterialSymbol, lit: bool) -> Button {
    button(icon(symbol).extent(Length::px(22.0)).color(match lit {
        true => palette().text,
        false => palette().text_muted,
    }))
    .icon_only()
    .ghost()
    .focusless()
}

pub(crate) fn view(chat: &Chat, channel: &ChannelId) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().height(HEIGHT).any();
    };
    let Some(channel_info) = linked.channel(channel) else {
        return spacer().height(HEIGHT).any();
    };
    let conversation = chat.conversation();
    let title = linked.channel_title(channel_info);
    let forum = channel_info.kind == ChannelKind::Forum;
    let guidelines = forum && channel_info.topic.is_some();
    let inline_topic = channel_info
        .topic
        .as_ref()
        .filter(|_| !forum)
        .map(Rich::one_line)
        .filter(|topic| !topic.is_empty());
    let bare = inline_topic.is_none();
    let parent = match channel_info.kind {
        ChannelKind::Thread => channel_info
            .parent
            .as_ref()
            .and_then(|parent| linked.channel(parent))
            .map(|parent| (parent.id, linked.channel_title(parent), parent.kind)),
        _ => None,
    };
    let private = channel_info.kind.is_private();
    let threaded = channel_info.space.is_some()
        && matches!(
            channel_info.kind,
            ChannelKind::Text | ChannelKind::Announcement
        );
    let unjoined = channel_info
        .thread
        .as_ref()
        .filter(|thread| !thread.joined && !thread.archived)
        .map(|_| channel_info.clone());
    let shown_members = conversation.members_shown();
    let shown = conversation.overlay().shown();
    let overlay_pins = matches!(shown, Some(Shown::Pins));
    let overlay_threads = matches!(shown, Some(Shown::Threads));
    let overlay_topic = matches!(shown, Some(Shown::Topic));
    let called = *channel;
    container(
        row! {
            if let Some((parent, name, kind)) = parent {
                container(
                    row! {
                        icon(symbol_of(kind)).extent(Length::px(20.0)).color(palette().text_muted);
                        text(name).label().semibold().muted().no_wrap();
                    }
                    .gap(Space::TIGHT)
                    .align_items(Align::Center),
                )
                .focusless()
                .on_press(move |chat: &mut Chat| chat.open_channel(&parent));
                icon(MaterialSymbol::CHEVRON_RIGHT).extent(Length::px(18.0)).color(palette().text_muted);
            }
            icon(symbol_of(channel_info.kind)).extent(Length::px(22.0)).color(palette().text_muted);
            text(title).label().semibold().no_wrap().min_width(0.0);
            if let Some(topic) = inline_topic {
                rule_upright().height(Length::px(20.0));
                container(text(topic).caption().muted().no_wrap().fill_width())
                    .focusless()
                    .on_press(|overlay: &mut Overlay| overlay.toggle(Shown::Topic))
                    .grow(1.0)
                    .min_width(0.0)
                    .build()
                    .keyed(TOPIC);
            }
            if bare {
                spacer().grow(1.0);
            }
            if let Some(thread) = unjoined {
                button(text(tr!("discord.conversation.join-thread")).caption())
                    .small()
                    .secondary()
                    .on_press(move |chat: &mut Chat| chat.join_thread(&thread));
            }
            if private {
                tool(MaterialSymbol::CALL, false)
                    .on_press(move |chat: &mut Chat| chat.join_voice(&called));
            }
            if threaded {
                tool(MaterialSymbol::FORUM, overlay_threads)
                    .on_press(|chat: &mut Chat| chat.show_threads())
                    .build()
                    .keyed(THREADS);
            }
            if guidelines {
                tool(MaterialSymbol::INFO, overlay_topic)
                    .on_press(|overlay: &mut Overlay| overlay.toggle(Shown::Topic))
                    .build()
                    .keyed(TOPIC);
            }
            if !forum {
                tool(MaterialSymbol::PUSH_PIN, overlay_pins)
                    .on_press(|chat: &mut Chat| chat.show_pins())
                    .build()
                    .keyed(PINS);
            }
            if !matches!(channel_info.kind, ChannelKind::Direct) {
                tool(MaterialSymbol::GROUP, shown_members)
                    .on_press(|conversation: &mut Conversation| conversation.toggle_members());
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::ROOMY, Space::NONE))
    .align_y(Align::Center)
    .height(HEIGHT)
    .fill_width()
    .any()
}

impl Chat {
    pub(crate) fn join_thread(&mut self, thread: &Channel) {
        let Some(key) = self.current_key().map(str::to_owned) else {
            return;
        };
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        let mut joined = thread.clone();
        if let Some(about) = joined.thread.as_mut() {
            about.joined = true;
        }
        let id = thread.id;
        spawn(
            async move { client.join_thread(&id, true).await },
            move |chat: &mut Chat, done: discord_core::Result<()>| match done {
                Ok(()) => {
                    if let Some(linked) = chat.linked_mut(&key) {
                        linked.apply(Update::Channel(joined.clone()));
                    }
                }
                Err(error) => chat.conversation_mut().complain(error.to_string()),
            },
        );
    }
}
