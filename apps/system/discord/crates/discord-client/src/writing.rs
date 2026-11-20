use discord_core::{Channel, Emoji, Mention, MessageId};

#[must_use]
pub fn mention(mention: &Mention) -> String {
    match mention {
        Mention::User(user) => format!("<@{user}>"),
        Mention::Role(role) => format!("<@&{role}>"),
        Mention::Channel(channel) => format!("<#{channel}>"),
        Mention::Everyone => "@everyone".to_owned(),
        Mention::Here => "@here".to_owned(),
    }
}

#[must_use]
pub fn emoji(emoji: &Emoji) -> String {
    match &emoji.id {
        Some(id) if emoji.is_animated() => format!("<a:{}:{id}>", emoji.name),
        Some(id) => format!("<:{}:{id}>", emoji.name),
        None => emoji.name.clone(),
    }
}

#[must_use]
pub fn message_link(channel: &Channel, message: &MessageId) -> String {
    let scope = channel
        .space
        .map_or_else(|| "@me".to_owned(), |space| space.to_string());
    format!(
        "https://discord.com/channels/{scope}/{}/{message}",
        channel.id
    )
}
