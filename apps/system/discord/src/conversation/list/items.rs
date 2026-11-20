use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use discord_core::{Message, MessageId};

use crate::conversation::time;

const GROUPED_FOR: i64 = 7 * 60 * 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Since {
    Start,
    After(MessageId),
}

#[derive(Clone, Debug)]
pub(crate) enum Item {
    Opening,
    Older,
    Day(i64),
    Unread,
    Message { message: Arc<Message>, head: bool },
}

impl Item {
    pub(crate) fn name(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        match self {
            Item::Opening => "opening".hash(&mut hasher),
            Item::Older => "older".hash(&mut hasher),
            Item::Day(at) => ("day", time::day_of(*at)).hash(&mut hasher),
            Item::Unread => "unread".hash(&mut hasher),
            Item::Message { message, .. } => ("message", &message.id).hash(&mut hasher),
        }
        hasher.finish()
    }

    pub(crate) fn message(&self) -> Option<&Arc<Message>> {
        match self {
            Item::Message { message, .. } => Some(message),
            _ => None,
        }
    }
}

pub(crate) fn items(messages: &[Arc<Message>], since: Option<&Since>, oldest: bool) -> Vec<Item> {
    let mut items = Vec::with_capacity(messages.len() + 4);
    items.push(match oldest {
        true => Item::Opening,
        false => Item::Older,
    });
    let unread_at = since.and_then(|since| unread_at(messages, since, oldest));
    let mut previous: Option<&Message> = None;
    let mut previous_day = None;
    for (at, message) in messages.iter().enumerate() {
        let day = time::day_of(message.at);
        let new_day = previous_day != Some(day);
        previous_day = Some(day);
        if new_day {
            items.push(Item::Day(message.at));
        }
        let unread = unread_at == Some(at);
        if unread {
            items.push(Item::Unread);
        }
        let head = new_day || unread || previous.is_none_or(|before| !follows(before, message));
        items.push(Item::Message {
            message: message.clone(),
            head,
        });
        previous = Some(message);
    }
    items
}

fn unread_at(messages: &[Arc<Message>], since: &Since, oldest: bool) -> Option<usize> {
    let at = match since {
        Since::Start => 0,
        Since::After(read) => match messages.iter().position(|message| &message.id == read) {
            Some(index) => index + 1,
            None if !oldest => 0,
            None => return None,
        },
    };
    (at < messages.len()).then_some(at)
}

fn follows(before: &Message, message: &Message) -> bool {
    before.author.id == message.author.id
        && !before.is_system()
        && !message.is_system()
        && message.reply.is_none()
        && message.at.saturating_sub(before.at) < GROUPED_FOR
}
