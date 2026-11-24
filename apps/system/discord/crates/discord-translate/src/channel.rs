use discord_core::{
    Abilities, Channel, ChannelId, ChannelKind, Emoji, ForumSettings, ForumTag, PostLayout,
    PostSort, SpaceId, TagId, TagMatch, Thread, UserId,
};

use super::{guild, instant};
use crate::cdn;
use discord_wire as wire;
use discord_wire::snowflake;
use discord_wire::{channel_flags, channel_kind, forum_layout, sort_order, tag_setting};

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
            pinned: wire.flags & channel_flags::PINNED != 0,
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
    let forum = (kind == ChannelKind::Forum).then(|| forum_settings(wire));
    let private = matches!(wire.kind, channel_kind::DIRECT | channel_kind::GROUP);
    Some(Channel {
        id: snowflake::id::<ChannelId>(wire.id.as_str()),
        space: space.map(snowflake::id::<SpaceId>),
        kind,
        name: wire.name.clone().unwrap_or_default(),
        topic: wire
            .topic
            .as_deref()
            .filter(|topic| !topic.trim().is_empty())
            .map(discord_markdown::parse),
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
        forum,
        bitrate: wire.bitrate,
        user_limit: wire.user_limit.filter(|&limit| limit > 0),
    })
}

fn forum_settings(wire: &wire::Channel) -> ForumSettings {
    ForumSettings {
        tags: wire
            .available_tags
            .iter()
            .map(|tag| ForumTag {
                id: snowflake::id::<TagId>(tag.id.as_str()),
                name: tag.name.clone(),
                emoji: emoji_of(tag.emoji_id.as_ref(), tag.emoji_name.as_ref()),
                moderated: tag.moderated,
            })
            .collect(),
        sort: match wire.default_sort_order {
            Some(sort_order::CREATION_DATE) => PostSort::Created,
            _ => PostSort::Activity,
        },
        layout: match wire.default_forum_layout {
            Some(forum_layout::GALLERY_VIEW) => PostLayout::Gallery,
            Some(forum_layout::LIST_VIEW) => PostLayout::List,
            _ if wire.kind == channel_kind::MEDIA => PostLayout::Gallery,
            _ => PostLayout::List,
        },
        tag_match: match wire.default_tag_setting.as_deref() {
            Some(tag_setting::MATCH_ALL) => TagMatch::All,
            _ => TagMatch::Some,
        },
        requires_tag: wire.flags & channel_flags::REQUIRE_TAG != 0,
        reaction: wire.default_reaction_emoji.as_ref().and_then(|reaction| {
            emoji_of(reaction.emoji_id.as_ref(), reaction.emoji_name.as_ref())
        }),
    }
}

fn emoji_of(id: Option<&String>, name: Option<&String>) -> Option<Emoji> {
    match (id, name) {
        (Some(id), name) => guild::custom_emoji(&wire::Emoji {
            id: Some(id.clone()),
            name: name.cloned(),
            animated: false,
        }),
        (None, Some(name)) if !name.is_empty() => Some(Emoji::unicode(name.as_str())),
        _ => None,
    }
}
