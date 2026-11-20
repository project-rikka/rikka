use std::sync::Arc;

use discord_core::{ChannelId, Emoji, Message, MessageId, ReactionChange, ReactionKind, Update};
use rikka_app::prelude::*;

use crate::chat::Chat;

impl Chat {
    fn locate(&self, message: &MessageId) -> Option<(String, ChannelId, Arc<Message>)> {
        let (key, channel) = self.conversation().shown()?.clone();
        let stored = self
            .linked(&key)?
            .timeline(&channel)?
            .find(message)?
            .clone();
        Some((key, channel, stored))
    }

    pub(crate) fn react(&mut self, message: &MessageId, emoji: &Emoji, on: bool) {
        let Some((key, channel, _)) = self.locate(message) else {
            return;
        };
        let Some(linked) = self.linked_mut(&key) else {
            return;
        };
        let change = |on: bool| {
            Update::Reaction(ReactionChange {
                channel,
                message: *message,
                kind: match on {
                    true => ReactionKind::Added {
                        emoji: emoji.clone(),
                        me: true,
                    },
                    false => ReactionKind::Removed {
                        emoji: emoji.clone(),
                        me: true,
                    },
                },
            })
        };
        linked.apply(change(on));
        let undo = change(!on);
        let client = linked.client().clone();
        let (channel_id, id, chosen) = (channel, *message, emoji.clone());
        spawn(
            async move { client.react(&channel_id, &id, &chosen, on).await },
            move |chat: &mut Chat, done: discord_core::Result<()>| {
                if let Err(error) = done {
                    if let Some(linked) = chat.linked_mut(&key) {
                        linked.apply(undo.clone());
                    }
                    chat.conversation_mut().complain(error.to_string());
                }
            },
        );
    }

    pub(crate) fn pin(&mut self, message: &MessageId, on: bool) {
        let Some((key, channel, original)) = self.locate(message) else {
            return;
        };
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        let id = *message;
        spawn(
            async move { client.pin(&channel, &id, on).await },
            move |chat: &mut Chat, done: discord_core::Result<()>| match done {
                Ok(()) => {
                    let mut pinned = Message::clone(&original);
                    pinned.pinned = on;
                    if let Some(linked) = chat.linked_mut(&key) {
                        linked.apply(Update::Message(pinned));
                    }
                }
                Err(error) => chat.conversation_mut().complain(error.to_string()),
            },
        );
    }

    pub(crate) fn ask_delete(&mut self, message: &MessageId, at_once: bool) {
        match at_once {
            true => self.delete_message(message),
            false => self.conversation_mut().overlay.ask_delete(*message),
        }
    }

    pub(crate) fn delete_message(&mut self, message: &MessageId) {
        self.conversation_mut().overlay.close();
        let Some((key, channel, original)) = self.locate(message) else {
            return;
        };
        let Some(linked) = self.linked_mut(&key) else {
            return;
        };
        linked.apply(Update::Deleted {
            channel,
            messages: vec![*message],
        });
        let client = linked.client().clone();
        let id = *message;
        spawn(
            async move { client.delete(&channel, &id).await },
            move |chat: &mut Chat, done: discord_core::Result<()>| {
                if let Err(error) = done {
                    if let Some(linked) = chat.linked_mut(&key) {
                        linked.apply(Update::Message(Message::clone(&original)));
                    }
                    chat.conversation_mut().complain(error.to_string());
                }
            },
        );
    }
}
