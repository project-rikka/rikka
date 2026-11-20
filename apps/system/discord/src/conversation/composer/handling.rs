use std::time::{Duration, Instant};

use discord_core::{Abilities, Emoji, Message, MessageId, Rich, Update};
use rikka_app::prelude::*;
use rikka_clipboard::URI_LIST;
use rikka_ui::{DragAction, DragOffer};

use super::Replying;
use super::complete::{self, Offers, Picked};
use super::uploads::{self, Attach, PickedFiles};
use crate::chat::Chat;

const TYPING_EVERY: Duration = Duration::from_secs(8);

impl Chat {
    pub(crate) fn composer_changed(&mut self) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        let draft = self.conversation().composer.draft.clone();
        let text = draft.text();
        let offers = self.linked(&key).and_then(|linked| {
            let channel_info = linked.channel(&channel)?;
            let typed = complete::typed_at(&text, draft.caret())?;
            let rows = complete::offers(linked, channel_info, &typed);
            (!rows.is_empty()).then_some(Offers {
                typed,
                rows,
                picked: 0,
            })
        });
        let composer = &mut self.conversation_mut().composer;
        if *composer.offers != offers {
            composer.offers.set(offers);
        }
        if composer.refused.is_some() {
            composer.refused.set(None);
        }
        let due = composer
            .typed_at
            .is_none_or(|at| at.elapsed() >= TYPING_EVERY);
        if text.trim().is_empty() || !due {
            return;
        }
        composer.typed_at = Some(Instant::now());
        if let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) {
            spawn(
                async move { client.typing(&channel).await },
                |_: discord_core::Result<()>| skip_view(),
            );
        }
    }

    pub(crate) fn composer_key(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let composer = &mut self.conversation_mut().composer;
        if composer.offers.is_some() && modifiers == Modifiers::NONE {
            match key {
                Key::ArrowUp => composer.step_offers(false),
                Key::ArrowDown => composer.step_offers(true),
                Key::Escape => composer.close_offers(),
                Key::Enter | Key::Tab => {
                    let picked = composer.offers.as_ref().map_or(0, |offers| offers.picked);
                    self.take_offer(picked);
                }
                _ => return false,
            }
            return true;
        }
        if modifiers != Modifiers::NONE {
            return false;
        }
        match key {
            Key::Escape if composer.reply.is_some() => {
                composer.cancel_reply();
                true
            }
            Key::ArrowUp if composer.draft.is_empty() => self.edit_last_own(),
            _ => false,
        }
    }

    pub(crate) fn take_offer(&mut self, index: usize) {
        let composer = &mut self.conversation_mut().composer;
        let Some(offers) = composer.offers.take() else {
            return;
        };
        let Some(offer) = offers.rows.get(index) else {
            return;
        };
        let draft = composer.draft.clone();
        draft.select(offers.typed.at, offers.typed.end.min(draft.len()));
        draft.insert(format!("{} ", offer.display));
        if let Some(picked) = offer.picked.clone() {
            composer.pick(offer.display.clone(), picked);
        }
    }

    pub(crate) fn insert_emoji(&mut self, emoji: &Emoji) {
        let display = match emoji.is_custom() {
            true => format!(":{}:", emoji.name),
            false => emoji.name.clone(),
        };
        let composer = &mut self.conversation_mut().composer;
        composer.draft.insert(&display);
        if emoji.is_custom() {
            composer.pick(display, Picked::Emoji(emoji.clone()));
        }
    }

    pub(crate) fn reply_to(&mut self, message: &MessageId) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        let Some(linked) = self.linked(&key) else {
            return;
        };
        let space = linked
            .channel(&channel)
            .and_then(|channel_info| channel_info.space);
        let Some(replied) = linked
            .timeline(&channel)
            .and_then(|timeline| timeline.find(message))
        else {
            return;
        };
        let author = linked.display_name(space.as_ref(), &replied.author);
        let mention = !linked.is_me(&replied.author.id);
        let composer = &mut self.conversation_mut().composer;
        composer.editing.set(None);
        composer.reply.set(Some(Replying {
            message: *message,
            author,
            mention,
        }));
    }

    pub(crate) fn start_edit(&mut self, message: &MessageId) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        let Some(text) = self
            .linked(&key)
            .and_then(|linked| linked.timeline(&channel))
            .and_then(|timeline| timeline.find(message))
            .map(|edited| edited.text.clone())
        else {
            return;
        };
        let composer = &mut self.conversation_mut().composer;
        composer.edit_draft.set_text(text);
        composer.editing.set(Some(*message));
    }

    fn edit_last_own(&mut self) -> bool {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return false;
        };
        let last = self.linked(&key).and_then(|linked| {
            linked
                .timeline(&channel)?
                .messages()
                .iter()
                .rev()
                .find(|message| {
                    linked.is_me(&message.author.id)
                        && !message.is_system()
                        && message.delivery == discord_core::Delivery::Delivered
                })
                .map(|message| message.id)
        });
        match last {
            Some(message) => {
                self.start_edit(&message);
                true
            }
            None => false,
        }
    }

    pub(crate) fn edit_key(&mut self, key: Key, modifiers: Modifiers) -> bool {
        if key == Key::Escape && modifiers == Modifiers::NONE {
            self.conversation_mut().composer.cancel_edit();
            return true;
        }
        false
    }

    pub(crate) fn save_edit(&mut self) {
        let Some((key, channel)) = self.conversation().shown().cloned() else {
            return;
        };
        let composer = &mut self.conversation_mut().composer;
        let Some(message) = composer.editing.take() else {
            return;
        };
        let text = composer.edit_draft.text().trim().to_owned();
        let Some(original) = self
            .linked(&key)
            .and_then(|linked| linked.timeline(&channel))
            .and_then(|timeline| timeline.find(&message))
            .cloned()
        else {
            return;
        };
        if text == original.text {
            return;
        }
        if text.is_empty() {
            self.ask_delete(&message, false);
            return;
        }
        let Some(linked) = self.linked_mut(&key) else {
            return;
        };
        let mut changed = Message::clone(&original);
        changed.text = text.clone();
        changed.content = Rich::plain(text.clone());
        changed.edited = Some(jiff::Timestamp::now().as_millisecond());
        linked.apply(Update::Message(changed));
        let client = linked.client().clone();
        let (landed_key, landed_channel) = (key.clone(), channel);
        spawn(
            async move { client.edit(&channel, &message, &text).await },
            move |chat: &mut Chat, edited: discord_core::Result<Message>| {
                chat.edited(&landed_key, &landed_channel, &original, edited);
            },
        );
    }

    pub(crate) fn edited(
        &mut self,
        key: &str,
        channel: &discord_core::ChannelId,
        original: &Message,
        edited: discord_core::Result<Message>,
    ) {
        let Some(linked) = self.linked_mut(key) else {
            return;
        };
        match edited {
            Ok(message) => {
                linked.apply(Update::Message(message));
            }
            Err(error) => {
                if original.channel == *channel {
                    linked.apply(Update::Message(original.clone()));
                }
                self.conversation_mut().complain(error.to_string());
            }
        }
    }

    pub(crate) fn attach(&mut self, to: Attach) {
        let parent = window_identifier();
        spawn_blocking(
            move || uploads::chosen(parent),
            move |chat: &mut Chat, picked: PickedFiles| {
                chat.attached(to, picked);
            },
        );
    }

    pub(crate) fn paste_into(&mut self, to: Attach, offered: Vec<String>) -> bool {
        let files = offered.iter().any(|mime| mime == URI_LIST);
        if uploads::picture_offered(&offered).is_none() && !files {
            return false;
        }
        if !self.may_attach() {
            return false;
        }
        spawn_blocking(
            move || uploads::pasted(offered),
            move |chat: &mut Chat, picked: PickedFiles| chat.attached(to, picked),
        );
        true
    }

    pub(crate) fn drop_into(&mut self, to: Attach, offer: DragOffer, _action: DragAction) {
        if !self.may_attach() {
            return;
        }
        spawn_blocking(
            move || uploads::dropped(&offer),
            move |chat: &mut Chat, picked: PickedFiles| chat.attached(to, picked),
        );
    }

    fn may_attach(&self) -> bool {
        let Some((key, channel)) = self.conversation().shown() else {
            return false;
        };
        self.linked(key)
            .and_then(|linked| linked.channel(channel))
            .is_some_and(|channel_info| channel_info.abilities.contains(Abilities::ATTACH))
    }

    pub(crate) fn attached(&mut self, to: Attach, picked: PickedFiles) {
        let Some((key, _)) = self.conversation().shown().cloned() else {
            return;
        };
        let Some(limits) = self.linked(&key).map(|linked| linked.client().limits()) else {
            return;
        };
        let conversation = self.conversation_mut();
        let mut refused = picked.refused;
        match to {
            Attach::Message => {
                let composer = &mut conversation.composer;
                let mut next = composer.next_upload;
                let (admitted, more) =
                    uploads::admit(&composer.uploads, picked.uploads, limits, &mut next);
                composer.next_upload = next;
                refused.extend(more);
                composer.admit(admitted, refused);
            }
            Attach::Post => conversation.forum.attached(picked.uploads, refused, limits),
        }
    }
}
