use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use discord_client::writing;
use discord_core::{
    Abilities, ChannelId, Draft, Message, MessageId, Progress, ReplyTo, Sticker, Update,
};
use rikka_app::prelude::*;
use rikka_ui::Element;
use rikka_widgets::row;

use crate::chat::Chat;
use crate::conversation::composer::{Picked, sendable};

pub(crate) struct Outgoing {
    key: String,
    channel: ChannelId,
    draft: Draft,
    shown: String,
    watchers: Watchers,
    done: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
}

#[derive(Default)]
pub(crate) struct Outbox(HashMap<MessageId, Outgoing>);

impl Outbox {
    pub(crate) fn progress(&self, nonce: &MessageId) -> Option<Element> {
        let outgoing = self.0.get(nonce)?;
        if outgoing.draft.uploads.is_empty() {
            return None;
        }
        let (done, total) = (outgoing.done.clone(), outgoing.total.clone());
        Some(
            row! {
                text(tr!(
                    "discord.conversation.uploading",
                    count = outgoing.draft.uploads.len() as i64
                ))
                .caption()
                .muted()
                .no_wrap();
                live_progress(&outgoing.watchers, move || {
                    let total = total.load(Ordering::Relaxed);
                    match total {
                        0 => 0.0,
                        _ => done.load(Ordering::Relaxed) as f32 / total as f32,
                    }
                })
                .grow(1.0);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .max_width(420.0)
            .any(),
        )
    }
}

pub(crate) fn wire(text: &str, picked: &[(String, Picked)]) -> String {
    let mut ordered: Vec<&(String, Picked)> = picked.iter().collect();
    ordered.sort_by_key(|(display, _)| Reverse(display.len()));
    let mut out = text.to_owned();
    for (display, picked) in ordered {
        let wired = match picked {
            Picked::Mention(mention) => writing::mention(mention),
            Picked::Emoji(emoji) => writing::emoji(emoji),
        };
        if wired != *display {
            out = out.replace(display.as_str(), &wired);
        }
    }
    out
}

impl Chat {
    pub(crate) fn send_draft(&mut self, channel: &ChannelId) {
        let Some(key) = self.current_key().map(str::to_owned) else {
            return;
        };
        let Some(linked) = self.linked(&key) else {
            return;
        };
        let Some(channel_info) = linked.channel(channel) else {
            return;
        };
        if !sendable(channel_info) {
            return;
        }
        let client = linked.client().clone();
        let bypass = channel_info.abilities.contains(Abilities::MANAGE_MESSAGES);
        let text = self.conversation().composer().text().trim().to_owned();
        let composer = &self.conversation().composer;
        if text.is_empty() && composer.uploads().is_empty() {
            return;
        }
        let limits = client.limits();
        if text.chars().count() > limits.message_chars {
            let caption = tr!(
                "discord.conversation.too-long",
                count = limits.message_chars as i64
            )
            .to_string();
            self.conversation_mut().composer.refused.set(Some(caption));
            return;
        }
        if composer.cooldown() > 0 && !bypass {
            return;
        }
        let wired = wire(&text, composer.picked());
        let reply = composer.reply().map(|reply| ReplyTo {
            message: reply.message,
            mention: reply.mention,
        });
        let composer = &mut self.conversation_mut().composer;
        let uploads = composer.take_uploads();
        composer.cleared();
        composer
            .sent_at
            .insert((key.clone(), *channel), std::time::Instant::now());
        let draft = Draft {
            nonce: client.mint(),
            text: wired,
            reply,
            uploads,
            stickers: Vec::new(),
            silent: false,
        };
        self.dispatch(&key, channel, draft, text);
    }

    pub(crate) fn send_sticker(&mut self, sticker: &Sticker) {
        self.conversation_mut().overlay.close();
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        let reply = self.conversation().composer.reply().map(|reply| ReplyTo {
            message: reply.message,
            mention: reply.mention,
        });
        self.conversation_mut().composer.cancel_reply();
        let draft = Draft {
            nonce: client.mint(),
            text: String::new(),
            reply,
            uploads: Vec::new(),
            stickers: vec![sticker.id],
            silent: false,
        };
        self.dispatch(&key, &channel, draft, sticker.name.clone());
    }

    pub(crate) fn say(&mut self, key: &str, channel: &ChannelId, caption: String) {
        let Some(client) = self.linked(key).map(|linked| linked.client().clone()) else {
            return;
        };
        let draft = Draft {
            nonce: client.mint(),
            text: caption.clone(),
            reply: None,
            uploads: Vec::new(),
            stickers: Vec::new(),
            silent: false,
        };
        self.dispatch(key, channel, draft, caption);
    }

    fn dispatch(&mut self, key: &str, channel: &ChannelId, draft: Draft, shown: String) {
        let Some(linked) = self.linked_mut(key) else {
            return;
        };
        let client = linked.client().clone();
        let pending = Draft {
            text: shown.clone(),
            ..draft.clone()
        };
        let watchers = Watchers::new();
        let (done, total) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicU64::new(0)));
        let outgoing = Outgoing {
            key: key.to_owned(),
            channel: *channel,
            draft: draft.clone(),
            shown,
            watchers: watchers.clone(),
            done: done.clone(),
            total: total.clone(),
        };
        let progress: Progress = Arc::new(move |sent, whole| {
            done.store(sent, Ordering::Relaxed);
            total.store(whole, Ordering::Relaxed);
            watchers.repaint();
        });
        let conversation = self.conversation_mut();
        conversation.outbox.0.insert(draft.nonce, outgoing);
        conversation.reading.read_own();
        if let Some(linked) = self.linked_mut(key) {
            linked.change(|store| vec![store.pend(channel, &pending)]);
        }
        self.jump_to_present();
        let (landed_key, landed_channel, nonce) = (key.to_owned(), *channel, draft.nonce);
        let to = *channel;
        spawn(
            async move { client.send(&to, draft, progress).await },
            move |chat: &mut Chat, sent: discord_core::Result<Message>| {
                chat.sent(&landed_key, &landed_channel, &nonce, sent);
            },
        );
    }

    pub(crate) fn sent(
        &mut self,
        key: &str,
        channel: &ChannelId,
        nonce: &MessageId,
        sent: discord_core::Result<Message>,
    ) {
        let Some(linked) = self.linked_mut(key) else {
            return;
        };
        match sent {
            Ok(message) => {
                linked.apply(Update::Message(message));
                self.conversation_mut().outbox.0.remove(nonce);
            }
            Err(error) => {
                tracing::warn!("failed to send message: {error}");
                linked.change(|store| store.fail(channel, nonce));
            }
        }
    }

    pub(crate) fn retry_message(&mut self, nonce: &MessageId) {
        let Some(outgoing) = self.conversation_mut().outbox.0.remove(nonce) else {
            return;
        };
        let Some(linked) = self.linked_mut(&outgoing.key) else {
            return;
        };
        linked.change(|store| store.withdraw(&outgoing.channel, nonce));
        let draft = Draft {
            nonce: linked.client().mint(),
            ..outgoing.draft
        };
        self.dispatch(&outgoing.key, &outgoing.channel, draft, outgoing.shown);
    }

    pub(crate) fn discard_message(&mut self, nonce: &MessageId) {
        let Some(outgoing) = self.conversation_mut().outbox.0.remove(nonce) else {
            return;
        };
        if let Some(linked) = self.linked_mut(&outgoing.key) {
            linked.change(|store| store.withdraw(&outgoing.channel, nonce));
        }
    }
}
