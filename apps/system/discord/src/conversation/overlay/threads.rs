use std::cmp::Reverse;

use discord_core::{Channel, ChannelId, MessageId, Update};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::{column, row};

use super::{Listing, Overlay, Shown};
use crate::chat::Chat;
use crate::conversation::header::THREADS;
use crate::conversation::{AREA, time};

const WIDTH: f32 = 420.0;
const HEIGHT: f32 = 460.0;
const NAMING: f32 = 380.0;

impl Chat {
    pub(crate) fn show_threads(&mut self) {
        let overlay = &mut self.conversation_mut().overlay;
        if matches!(*overlay.shown, Some(Shown::Threads)) {
            overlay.close();
            return;
        }
        overlay.shown.set(Some(Shown::Threads));
        let archived = *overlay.archived;
        self.load_threads(archived);
    }

    pub(crate) fn pick_thread_list(&mut self, archived: bool) {
        self.conversation_mut().overlay.show_archived(archived);
        self.load_threads(archived);
    }

    fn load_threads(&mut self, archived: bool) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        self.conversation_mut()
            .overlay
            .threads
            .set(Listing::Loading);
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        let channel_id = channel;
        spawn_keyed(
            ("discord-threads", key.clone(), channel),
            async move { client.threads(&channel_id, archived).await },
            move |chat: &mut Chat, threads: discord_core::Result<Vec<Channel>>| {
                chat.on_threads_loaded(&key, &channel, archived, threads);
            },
        );
    }

    pub(crate) fn on_threads_loaded(
        &mut self,
        key: &str,
        channel: &ChannelId,
        archived: bool,
        threads: discord_core::Result<Vec<Channel>>,
    ) {
        let location = self
            .conversation()
            .shown()
            .is_some_and(|(shown, at)| shown == key && at == channel);
        if !location || self.conversation().overlay().archived() != archived {
            return;
        }
        let listing = match threads {
            Ok(mut threads) => {
                threads.sort_by_key(|thread| Reverse(thread.last_activity));
                Listing::Ready(threads)
            }
            Err(error) => Listing::Failed(error.to_string()),
        };
        self.conversation_mut().overlay.threads.set(listing);
    }

    pub(crate) fn open_thread(&mut self, key: &str, thread: Channel) {
        let id = thread.id;
        if let Some(linked) = self.linked_mut(key)
            && linked.channel(&id).is_none()
        {
            linked.apply(Update::Channel(thread));
        }
        self.conversation_mut().overlay.close();
        self.open_channel(&id);
    }

    pub(crate) fn create_thread(&mut self, from: Option<MessageId>) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        let name = self
            .conversation()
            .overlay()
            .thread_name()
            .text()
            .trim()
            .to_owned();
        if name.is_empty() {
            return;
        }
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        self.conversation_mut().overlay.creating.set(true);
        spawn(
            async move { client.create_thread(&channel, from.as_ref(), &name).await },
            move |chat: &mut Chat, created: discord_core::Result<Channel>| {
                chat.thread_created(&key, created);
            },
        );
    }

    pub(crate) fn thread_created(&mut self, key: &str, created: discord_core::Result<Channel>) {
        self.conversation_mut().overlay.creating.set(false);
        match created {
            Ok(thread) => self.open_thread(key, thread),
            Err(error) => {
                self.conversation_mut().overlay.close();
                self.conversation_mut().complain(error.to_string());
            }
        }
    }
}

pub(super) fn view(chat: &Chat) -> Element {
    let overlay = chat.conversation().overlay();
    let archived = overlay.archived();
    let key = chat
        .conversation()
        .shown()
        .map(|(key, _)| key.clone())
        .unwrap_or_default();
    let body = match overlay.threads() {
        Listing::Loading => container(spinner()).center().fill().any(),
        Listing::Failed(caption) => status_page(MaterialSymbol::ERROR, caption.clone())
            .tint(palette().danger)
            .fill()
            .any(),
        Listing::Ready(threads) if threads.is_empty() => status_page(
            MaterialSymbol::FORUM,
            tr!("discord.conversation.no-threads"),
        )
        .fill()
        .any(),
        Listing::Ready(threads) => {
            let rows: Vec<Element> = threads
                .iter()
                .map(|thread| thread_row(&key, thread))
                .collect();
            scrollable(column_of(rows).gap(Space::HAIR).fill_width())
                .fill()
                .any()
        }
    };
    popover(
        column! {
            row! {
                text(tr!("discord.conversation.threads")).heading().semibold().grow(1.0);
                button(text(tr!("discord.conversation.create-thread")))
                    .small()
                    .primary()
                    .on_press(|overlay: &mut Overlay| overlay.ask_new_thread(None, ""));
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            segmented(usize::from(archived))
                .segment(tr!("discord.conversation.active-threads"))
                .segment(tr!("discord.conversation.archived-threads"))
                .on_select(|chat: &mut Chat, picked: usize| chat.pick_thread_list(picked == 1))
                .fill_width();
            body;
        }
        .gap(Space::NORMAL)
        .size(Length::px(WIDTH), Length::px(HEIGHT)),
    )
    .on_dismiss(|overlay: &mut Overlay| overlay.close())
    .build()
    .anchored(Anchor::new(THREADS, AnchorEdge::BelowEnd).offset(0.0, Space::TIGHT.px()))
}

fn thread_row(key: &str, thread: &Channel) -> Element {
    let count = thread.thread.as_ref().map_or(0, |about| about.messages);
    let active = thread.last_activity.map(time::sent_at).unwrap_or_default();
    let to_open = thread.clone();
    let key = key.to_owned();
    container(
        row! {
            icon(MaterialSymbol::FORUM).extent(Length::px(20.0)).color(palette().text_muted);
            column! {
                text(thread.name.clone()).label().semibold().no_wrap().fill_width();
                text(tr!("discord.conversation.thread-summary", count = i64::from(count), at = active.as_str()))
                    .caption()
                    .muted()
                    .no_wrap()
                    .fill_width();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::all(Space::SNUG))
    .radius(Corner::Item)
    .focusless()
    .on_press(move |chat: &mut Chat| chat.open_thread(&key, to_open.clone()))
    .fill_width()
    .any()
}

pub(super) fn naming(chat: &Chat, from: Option<&MessageId>) -> Element {
    let overlay = chat.conversation().overlay();
    let creating = overlay.creating();
    let made = from.cloned();
    let submitted = from.cloned();
    popover(
        column! {
            text(tr!("discord.conversation.new-thread")).heading().semibold();
            text_field(overlay.thread_name())
                .placeholder(tr!("discord.conversation.thread-name"))
                .autofocus(true)
                .on_submit(move |chat: &mut Chat| chat.create_thread(submitted))
                .fill_width();
            row! {
                spacer().grow(1.0);
                button(text(tr!("discord.conversation.cancel")))
                    .ghost()
                    .on_press(|overlay: &mut Overlay| overlay.close());
                button(text(tr!("discord.conversation.create")))
                    .primary()
                    .loading(creating)
                    .on_press(move |chat: &mut Chat| chat.create_thread(made));
            }
            .gap(Space::SNUG)
            .fill_width();
        }
        .gap(Space::NORMAL)
        .width(NAMING),
    )
    .on_dismiss(|overlay: &mut Overlay| overlay.close())
    .build()
    .anchored(Anchor::centered_on(AREA))
}
