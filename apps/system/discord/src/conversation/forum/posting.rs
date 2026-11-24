use discord_core::{Channel, ChannelId, Draft, Emoji, Upload, unwatched};
use rikka_app::prelude::*;

use crate::chat::Chat;
use crate::conversation::composer::write_emoji;
use crate::conversation::sending::wire;

impl Chat {
    pub(crate) fn start_post(&mut self) {
        let state = &mut self.conversation_mut().forum;
        let title = state.search.text().trim().to_owned();
        state.title.set_text(&title);
        state.writing.set(true);
        state.refill();
    }

    pub(crate) fn insert_post_emoji(&mut self, emoji: &Emoji) {
        let state = &mut self.conversation_mut().forum;
        write_emoji(&state.body, &mut state.picked, emoji);
        state.refill();
    }

    pub(crate) fn open_post(&mut self, thread: &ChannelId) {
        let Some((key, _)) = self.conversation().forum.seat.clone() else {
            return;
        };
        let known = self
            .linked(&key)
            .and_then(|linked| linked.channel(thread))
            .or_else(|| {
                self.conversation()
                    .forum
                    .posts
                    .list
                    .iter()
                    .find(|post| post.thread.id == *thread)
                    .map(|post| &post.thread)
            })
            .cloned();
        if let Some(channel) = known {
            self.open_thread(&key, channel);
        }
    }

    pub(crate) fn submit_post(&mut self) {
        let Some(((key, forum), ready)) = self
            .shown_forum()
            .map(|(seat, settings)| (seat, self.conversation().forum.ready(settings)))
        else {
            return;
        };
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        let state = &mut self.conversation_mut().forum;
        if *state.posting || !ready {
            return;
        }
        let title = state.title.text().trim().to_owned();
        let text = wire(state.body.text().trim(), &state.picked);
        let tags = state.chosen.to_vec();
        let uploads: Vec<Upload> = state
            .uploads
            .iter()
            .map(|pending| pending.upload.clone())
            .collect();
        state.posting.set(true);
        state.refused.set(None);
        let draft = Draft {
            nonce: client.mint(),
            text,
            reply: None,
            uploads,
            stickers: Vec::new(),
            silent: false,
        };
        let landed_key = key.clone();
        spawn(
            async move {
                client
                    .create_post(&forum, &title, &tags, draft, unwatched())
                    .await
            },
            move |chat: &mut Chat, posted: discord_core::Result<Channel>| {
                chat.posted(&landed_key, &forum, posted);
            },
        );
    }

    fn posted(&mut self, key: &str, forum: &ChannelId, posted: discord_core::Result<Channel>) {
        let seat = (key.to_owned(), *forum);
        let state = &mut self.conversation_mut().forum;
        if state.seat.as_ref() != Some(&seat) {
            return;
        }
        match posted {
            Ok(thread) => {
                state.cancel();
                state.seat = None;
                self.open_thread(key, thread);
            }
            Err(error) => {
                state.posting.set(false);
                state.refused.set(Some(error.to_string()));
            }
        }
    }

    pub(crate) fn react_to_post(&mut self, thread: ChannelId, emoji: Emoji, on: bool) {
        let Some((key, _)) = self.conversation().forum.seat.clone() else {
            return;
        };
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        let Some(starter) = self.conversation_mut().forum.react(&thread, &emoji, on) else {
            return;
        };
        let undone = emoji.clone();
        spawn(
            async move { client.react(&thread, &starter, &emoji, on).await },
            move |chat: &mut Chat, done: discord_core::Result<()>| {
                if let Err(error) = done {
                    chat.conversation_mut().forum.react(&thread, &undone, !on);
                    chat.conversation_mut().complain(error.to_string());
                }
            },
        );
    }
}
