use std::sync::Arc;

use crate::id::MessageId;
use crate::model::message::{Delivery, Embed, Message};
use crate::requests::Page;
use crate::update::ReactionKind;

const KEPT_MESSAGES: usize = 1000;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timeline {
    messages: Vec<Arc<Message>>,
    oldest: bool,
    newest: bool,
}

impl Timeline {
    #[must_use]
    pub fn messages(&self) -> &[Arc<Message>] {
        &self.messages
    }

    #[must_use]
    pub fn reached_oldest(&self) -> bool {
        self.oldest
    }

    #[must_use]
    pub fn reached_newest(&self) -> bool {
        self.newest
    }

    #[must_use]
    pub fn first(&self) -> Option<&MessageId> {
        self.messages
            .iter()
            .find(|message| message.delivery == Delivery::Delivered)
            .map(|message| &message.id)
    }

    #[must_use]
    pub fn last(&self) -> Option<&MessageId> {
        self.messages
            .iter()
            .rev()
            .find(|message| message.delivery == Delivery::Delivered)
            .map(|message| &message.id)
    }

    #[must_use]
    pub fn find(&self, id: &MessageId) -> Option<&Arc<Message>> {
        let at = self.at(id)?;
        Some(&self.messages[at])
    }

    pub(crate) fn absorb(&mut self, page: &Page, fetched: Vec<Message>) {
        let fetched_nothing = fetched.is_empty();
        let mut fetched: Vec<Arc<Message>> = fetched.into_iter().map(Arc::new).collect();
        fetched.sort_by_key(|message| message.id);
        match page {
            Page::Latest => {
                let pending: Vec<Arc<Message>> = self
                    .messages
                    .drain(..)
                    .filter(|message| message.delivery != Delivery::Delivered)
                    .collect();
                self.messages = fetched;
                self.oldest = fetched_nothing;
                self.newest = true;
                for message in pending {
                    self.place(message);
                }
            }
            Page::Around(_) => {
                self.messages = fetched;
                self.oldest = false;
                self.newest = false;
            }
            Page::Before(_) => {
                if fetched_nothing {
                    self.oldest = true;
                }
                for message in fetched {
                    self.place(message);
                }
                if self.messages.len() > KEPT_MESSAGES {
                    self.messages.truncate(KEPT_MESSAGES);
                    self.newest = false;
                }
            }
            Page::After(_) => {
                if fetched_nothing {
                    self.newest = true;
                }
                for message in fetched {
                    self.place(message);
                }
                self.trim_oldest();
            }
        }
    }

    pub(crate) fn arrive(&mut self, message: Message) -> bool {
        let message = Arc::new(message);
        if let Some(at) = self.position_of(&message) {
            self.messages.remove(at);
            self.place(message);
            return true;
        }
        if self.newest {
            self.place(message);
            self.trim_oldest();
            return true;
        }
        false
    }

    fn trim_oldest(&mut self) {
        let over = self.messages.len().saturating_sub(KEPT_MESSAGES);
        if over > 0 {
            self.messages.drain(..over);
            self.oldest = false;
        }
    }

    pub(crate) fn pend(&mut self, message: Message) {
        self.place(Arc::new(message));
    }

    pub(crate) fn fail(&mut self, nonce: &MessageId) -> bool {
        let Some(message) = self
            .messages
            .iter_mut()
            .find(|message| message.nonce.as_ref() == Some(nonce))
        else {
            return false;
        };
        Arc::make_mut(message).delivery = Delivery::Failed;
        true
    }

    pub(crate) fn withdraw(&mut self, nonce: &MessageId) -> bool {
        let before = self.messages.len();
        self.messages.retain(|message| {
            message.delivery == Delivery::Delivered || message.nonce.as_ref() != Some(nonce)
        });
        before != self.messages.len()
    }

    pub(crate) fn delete(&mut self, ids: &[MessageId]) -> bool {
        let before = self.messages.len();
        self.messages.retain(|message| !ids.contains(&message.id));
        before != self.messages.len()
    }

    pub(crate) fn embed(&mut self, id: &MessageId, embeds: Vec<Embed>) -> bool {
        self.edit(id, |message| message.embeds = embeds)
    }

    pub(crate) fn react(&mut self, id: &MessageId, kind: ReactionKind) -> bool {
        self.edit(id, |message| kind.apply(&mut message.reactions))
    }

    fn edit(&mut self, id: &MessageId, change: impl FnOnce(&mut Message)) -> bool {
        let Some(at) = self.at(id) else {
            return false;
        };
        change(Arc::make_mut(&mut self.messages[at]));
        true
    }

    fn at(&self, id: &MessageId) -> Option<usize> {
        self.messages
            .binary_search_by_key(id, |message| message.id)
            .ok()
    }

    fn position_of(&self, message: &Message) -> Option<usize> {
        self.messages.iter().position(|known| {
            known.id == message.id
                || (known.delivery != Delivery::Delivered
                    && message.nonce.is_some()
                    && known.nonce == message.nonce)
        })
    }

    fn place(&mut self, message: Arc<Message>) {
        match self
            .messages
            .binary_search_by_key(&message.id, |known| known.id)
        {
            Ok(at) => self.messages[at] = message,
            Err(at) => self.messages.insert(at, message),
        }
    }
}
