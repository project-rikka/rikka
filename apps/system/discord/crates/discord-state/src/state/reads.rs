use std::collections::HashMap;

use discord_core::{ChannelId, MessageId, Unread};

use discord_wire::ReadState;
use discord_wire::snowflake;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Entry {
    last_read: Option<String>,
    mentions: u32,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Reads {
    entries: HashMap<String, Entry>,
}

pub(crate) struct ChannelActivity<'a> {
    pub(crate) channel: &'a str,
    pub(crate) last_message: Option<&'a str>,
    pub(crate) tracked_since: Option<i64>,
}

impl Reads {
    pub(crate) fn load(states: &[ReadState]) -> Reads {
        let entries = states
            .iter()
            .filter(|state| state.read_state_type == 0)
            .map(|state| {
                let entry = Entry {
                    last_read: state.last_message_id.clone(),
                    mentions: state.mention_count,
                };
                (state.id.clone(), entry)
            })
            .collect();
        Reads { entries }
    }

    pub(crate) fn unread(&self, activity: &ChannelActivity<'_>) -> Unread {
        let entry = self.entries.get(activity.channel);
        let last_read = entry.and_then(|entry| entry.last_read.as_deref());
        let unread = match (activity.last_message, last_read) {
            (None, _) => false,
            (Some(last), Some(read)) => snowflake::newer(last, read),
            (Some(last), None) => activity
                .tracked_since
                .is_some_and(|since| snowflake::millis(last).is_some_and(|sent| sent > since)),
        };
        Unread {
            channel: snowflake::id::<ChannelId>(activity.channel),
            unread,
            mentions: entry.map_or(0, |entry| entry.mentions),
            last_read: last_read.map(snowflake::id::<MessageId>),
        }
    }

    pub(crate) fn mentioned(&mut self, channel: &str) {
        self.entries.entry(channel.to_owned()).or_default().mentions += 1;
    }

    pub(crate) fn read(&mut self, channel: &str, message: Option<&str>, mentions: u32) {
        let entry = self.entries.entry(channel.to_owned()).or_default();
        if let Some(message) = message
            && entry
                .last_read
                .as_deref()
                .is_none_or(|known| !snowflake::newer(known, message))
        {
            entry.last_read = Some(message.to_owned());
        }
        entry.mentions = mentions;
    }

    pub(crate) fn rewind(&mut self, channel: &str, message: Option<&str>, mentions: u32) {
        let entry = self.entries.entry(channel.to_owned()).or_default();
        entry.last_read = message.map(str::to_owned);
        entry.mentions = mentions;
    }

    pub(crate) fn has_mentions(&self, channel: &str) -> bool {
        self.entries
            .get(channel)
            .is_some_and(|entry| entry.mentions > 0)
    }
}
