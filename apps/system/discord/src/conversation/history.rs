use discord_core::{ChannelId, ChannelKind, Message, MessageId, Page};
use rikka_app::prelude::*;

use crate::chat::Chat;
use crate::conversation::Seat;
use crate::conversation::list::JumpTarget;
use crate::conversation::list::items::Since;

const HISTORY: &str = "discord-history";

impl Chat {
    pub(crate) fn entered(&mut self) {
        let shown: Option<(Seat, ChannelKind)> = self.current().and_then(|linked| {
            let channel = linked.location().channel?;
            let kind = linked.channel(&channel)?.kind;
            let shows = kind.holds_messages() || kind == ChannelKind::Forum;
            shows.then(|| ((linked.key().to_owned(), channel), kind))
        });
        let seat = shown.as_ref().map(|(seat, _)| seat.clone());
        let moved = self.conversation_mut().show(seat.clone());
        let Some(((key, channel), kind)) = shown else {
            return;
        };
        self.recall_recents(&key);
        self.start_ticking();
        if moved {
            let since = self.linked(&key).and_then(|linked| {
                linked
                    .unseen(&channel)
                    .unread
                    .then(|| match linked.last_read(&channel) {
                        Some(read) => Since::After(*read),
                        None => Since::Start,
                    })
            });
            self.conversation_mut().reading.reset(since);
        }
        if kind == ChannelKind::Forum {
            self.open_forum(&key, &channel);
            return;
        }
        let known = self
            .linked(&key)
            .is_some_and(|linked| linked.timeline(&channel).is_some());
        match known {
            true => {
                if let Some(linked) = self.linked_mut(&key) {
                    linked.change(|store| {
                        store.visit(&channel);
                        Vec::new()
                    });
                }
            }
            false => self.load_history(&key, &channel, Page::Latest),
        }
    }

    pub(crate) fn load_older(&mut self, channel: &ChannelId) {
        let Some(linked) = self.current() else {
            return;
        };
        let Some(timeline) = linked.timeline(channel) else {
            return;
        };
        if timeline.reached_oldest() {
            return;
        }
        let Some(first) = timeline.first().cloned() else {
            return;
        };
        let key = linked.key().to_owned();
        self.load_history(&key, channel, Page::Before(first));
    }

    pub(crate) fn load_newer(&mut self, channel: &ChannelId) {
        let Some(linked) = self.current() else {
            return;
        };
        let Some(timeline) = linked.timeline(channel) else {
            return;
        };
        if timeline.reached_newest() {
            return;
        }
        let Some(last) = timeline.last().cloned() else {
            return;
        };
        let key = linked.key().to_owned();
        self.load_history(&key, channel, Page::After(last));
    }

    fn load_history(&mut self, key: &str, channel: &ChannelId, page: Page) {
        let Some(client) = self.linked(key).map(|linked| linked.client().clone()) else {
            return;
        };
        let seat = (key.to_owned(), *channel);
        let loading = &mut self.conversation_mut().loading;
        let fresh = loading.insert(seat.clone());
        let replaces = matches!(page, Page::Latest | Page::Around(_));
        if !fresh && !replaces {
            return;
        }
        let request = page;
        let channel_id = *channel;
        spawn_keyed(
            (HISTORY, seat.clone()),
            async move { client.messages(&channel_id, request).await },
            move |chat: &mut Chat, fetched: discord_core::Result<Vec<Message>>| {
                chat.on_history_loaded(&seat.0, &seat.1, &page, fetched);
            },
        );
    }

    pub(crate) fn on_history_loaded(
        &mut self,
        key: &str,
        channel: &ChannelId,
        page: &Page,
        fetched: discord_core::Result<Vec<Message>>,
    ) {
        let seat = (key.to_owned(), *channel);
        self.conversation_mut().loading.remove(&seat);
        match fetched {
            Ok(messages) => {
                if let Some(linked) = self.linked_mut(key) {
                    linked.change(|store| store.absorb(channel, page, messages));
                }
                if self.conversation().shown() == Some(&seat) {
                    self.reveal_jump(&seat, page);
                }
            }
            Err(error) => {
                if self.current_key() == Some(key) {
                    self.conversation_mut().complain(error.to_string());
                }
            }
        }
    }

    fn reveal_jump(&mut self, seat: &Seat, page: &Page) {
        let jump = match page {
            Page::Around(_) | Page::Latest => self.conversation_mut().reading.take_jump(),
            _ => None,
        };
        let target = match jump {
            Some(JumpTarget::Message(message)) => Some((message, true)),
            Some(JumpTarget::Present) => self
                .linked(&seat.0)
                .and_then(|linked| linked.timeline(&seat.1))
                .and_then(|timeline| timeline.messages().last())
                .map(|message| (message.id, false)),
            None => None,
        };
        if let Some((message, flash)) = target {
            self.conversation_mut().reading.reveal(message, flash);
        }
    }

    pub(crate) fn jump_to(&mut self, message: &MessageId) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        let known = self
            .linked(&key)
            .and_then(|linked| linked.timeline(&channel))
            .is_some_and(|timeline| timeline.find(message).is_some());
        if known {
            self.conversation_mut().reading.reveal(*message, true);
            return;
        }
        self.conversation_mut()
            .reading
            .request_jump(JumpTarget::Message(*message));
        self.load_history(&key, &channel, Page::Around(*message));
    }

    pub(crate) fn jump_from_overlay(&mut self, message: &MessageId) {
        self.conversation_mut().overlay.close();
        self.jump_to(message);
    }

    pub(crate) fn jump_to_present(&mut self) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        let Some(timeline) = self
            .linked(&key)
            .and_then(|linked| linked.timeline(&channel))
        else {
            return;
        };
        match timeline.reached_newest() {
            true => self.conversation().reading.place().end(),
            false => {
                self.conversation_mut()
                    .reading
                    .request_jump(JumpTarget::Present);
                self.load_history(&key, &channel, Page::Latest);
            }
        }
    }
}
