use discord_core::{
    Abilities, Channel, ChannelId, ChannelKind, ForumTag, SpaceId, TagId, Thread, UserId,
};

use super::{guild, instant};
use crate::cdn;
use discord_wire as wire;
use discord_wire::channel_kind;
use discord_wire::snowflake;

#[derive(Clone, Copy, Debug)]
pub struct Looks {
    pub abilities: Abilities,
    pub muted: bool,
    pub joined: bool,
}

pub fn channel(wire: &wire::Channel, space: Option<&str>, looks: Looks) -> Option<Channel> {
    let kind = match wire.kind {
        channel_kind::TEXT => ChannelKind::Text,
        channel_kind::DIRECT => ChannelKind::Direct,
        channel_kind::VOICE => ChannelKind::Voice,
        channel_kind::GROUP => ChannelKind::Group,
        channel_kind::CATEGORY => ChannelKind::Category,
        channel_kind::ANNOUNCEMENT => ChannelKind::Announcement,
        channel_kind::NEWS_THREAD | channel_kind::PUBLIC_THREAD | channel_kind::PRIVATE_THREAD => {
            ChannelKind::Thread
        }
        channel_kind::STAGE => ChannelKind::Stage,
        channel_kind::FORUM | channel_kind::MEDIA => ChannelKind::Forum,
        _ => return None,
    };
    let thread = wire.is_thread().then(|| {
        let metadata = wire.thread_metadata.clone().unwrap_or_default();
        Thread {
            owner: wire.owner_id.as_deref().map(snowflake::id::<UserId>),
            archived: metadata.archived,
            locked: metadata.locked,
            joined: looks.joined,
            messages: wire.message_count.unwrap_or(0),
            members: wire.member_count.unwrap_or(0),
            applied_tags: wire
                .applied_tags
                .iter()
                .map(|tag| snowflake::id::<TagId>(tag.as_str()))
                .collect(),
            created: instant(metadata.create_timestamp.as_deref())
                .or_else(|| snowflake::millis(&wire.id)),
        }
    });
    let tags = wire
        .available_tags
        .iter()
        .map(|tag| ForumTag {
            id: snowflake::id::<TagId>(tag.id.as_str()),
            name: tag.name.clone(),
            emoji: match (&tag.emoji_id, &tag.emoji_name) {
                (Some(id), name) => guild::custom_emoji(&wire::Emoji {
                    id: Some(id.clone()),
                    name: name.clone(),
                    animated: false,
                }),
                (None, Some(name)) if !name.is_empty() => {
                    Some(discord_core::Emoji::unicode(name.as_str()))
                }
                _ => None,
            },
            moderated: tag.moderated,
        })
        .collect();
    let private = matches!(wire.kind, channel_kind::DIRECT | channel_kind::GROUP);
    Some(Channel {
        id: snowflake::id::<ChannelId>(wire.id.as_str()),
        space: space.map(snowflake::id::<SpaceId>),
        kind,
        name: wire.name.clone().unwrap_or_default(),
        topic: wire.topic.clone().filter(|topic| !topic.is_empty()),
        parent: wire.parent_id.as_deref().map(snowflake::id::<ChannelId>),
        position: wire.position,
        icon: wire
            .icon
            .as_deref()
            .filter(|_| private)
            .map(|hash| cdn::channel_icon(&wire.id, hash)),
        recipients: wire
            .recipients()
            .into_iter()
            .map(|user| snowflake::id::<UserId>(&user))
            .collect(),
        abilities: looks.abilities,
        nsfw: wire.nsfw,
        muted: looks.muted,
        slowmode: wire.rate_limit_per_user.unwrap_or(0),
        last_activity: wire
            .last_message_id
            .as_deref()
            .and_then(snowflake::millis)
            .or_else(|| thread.as_ref().and_then(|thread| thread.created)),
        thread,
        tags,
        bitrate: wire.bitrate,
        user_limit: wire.user_limit.filter(|&limit| limit > 0),
    })
}
