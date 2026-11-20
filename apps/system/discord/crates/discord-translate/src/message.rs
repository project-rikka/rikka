use std::sync::Arc;

use discord_core::{
    Attachment, AttachmentKind, ChannelId, Delivery, Embed, EmbedField, EmbedKind, EmbedLine,
    EmbedMedia, Message, MessageId, MessageKind, Picture, Quoted, Reaction, Reply, Rich,
    SystemEvent, UserId,
};

use super::{guild, instant, user};
use crate::cdn;
use discord_wire as wire;
use discord_wire::message_flags;
use discord_wire::snowflake;

const SPOILER_PREFIX: &str = "SPOILER_";
const SPOILER_FLAG: u64 = 1 << 3;
const FORWARD: u8 = 1;

#[derive(Clone, Copy, Debug)]
pub struct Reader<'a> {
    pub me: &'a str,
    pub roles: &'a [String],
    pub suppress_everyone: bool,
    pub suppress_roles: bool,
}

impl Reader<'_> {
    pub fn directly(&self, wire: &wire::Message) -> bool {
        wire.mentions
            .iter()
            .any(|mentioned| mentioned.id == self.me)
    }

    pub fn by_role(&self, wire: &wire::Message) -> bool {
        wire.mention_roles
            .iter()
            .any(|role| self.roles.contains(role))
    }

    pub fn mentioned(&self, wire: &wire::Message) -> bool {
        self.directly(wire)
            || (wire.mention_everyone && !self.suppress_everyone)
            || (self.by_role(wire) && !self.suppress_roles)
    }
}

pub fn message(wire: &wire::Message, reader: &Reader<'_>) -> Option<Message> {
    let author = wire.author.as_ref()?;
    let forwarded = (wire
        .message_reference
        .as_ref()
        .is_some_and(|reference| reference.kind == FORWARD))
    .then(|| {
        wire.message_snapshots
            .first()
            .map(|snapshot| &snapshot.message)
    })
    .flatten();
    let body = forwarded.unwrap_or(wire);
    let text = body.content.clone().unwrap_or_default();
    let kind = kind(wire);
    let content = match kind {
        MessageKind::Normal => discord_markdown::parse(&text),
        MessageKind::System(_) => Rich::default(),
    };
    Some(Message {
        id: snowflake::id::<MessageId>(wire.id.as_str()),
        channel: snowflake::id::<ChannelId>(wire.channel_id.as_str()),
        author: user::user(author),
        at: instant(wire.timestamp.as_deref())
            .or_else(|| snowflake::millis(&wire.id))
            .unwrap_or_default(),
        edited: instant(wire.edited_timestamp.as_deref()),
        kind,
        text,
        content,
        attachments: body.attachments.iter().map(attachment).collect(),
        embeds: embeds(body.embeds.as_deref().unwrap_or_default()),
        stickers: body
            .sticker_items
            .iter()
            .map(|item| guild::sticker(&item.id, &item.name, item.format_type, None))
            .collect(),
        reactions: wire
            .reactions
            .iter()
            .map(|reaction| Reaction {
                emoji: guild::reaction_emoji(&reaction.emoji),
                count: reaction.count,
                me: reaction.me || reaction.me_burst,
            })
            .collect(),
        reply: reply(wire),
        mentioned: wire.mentions.iter().map(user::user).collect(),
        mentions_me: reader.mentioned(wire),
        pinned: wire.pinned,
        thread: wire
            .thread
            .as_ref()
            .map(|thread| snowflake::id::<ChannelId>(thread.id.as_str()))
            .or_else(|| {
                (wire.flags & message_flags::HAS_THREAD != 0)
                    .then(|| snowflake::id::<ChannelId>(wire.id.as_str()))
            }),
        nonce: wire.nonce.as_deref().and_then(|nonce| nonce.parse().ok()),
        delivery: Delivery::Delivered,
    })
}

fn kind(wire: &wire::Message) -> MessageKind {
    let mentioned = || {
        wire.mentions.first().map_or_else(
            || snowflake::id::<UserId>(""),
            |user| snowflake::id::<UserId>(user.id.as_str()),
        )
    };
    let event = match wire.kind {
        0 | 19 | 20 | 21 | 23 => return MessageKind::Normal,
        1 => SystemEvent::RecipientAdded(mentioned()),
        2 => SystemEvent::RecipientRemoved(mentioned()),
        3 => SystemEvent::Call {
            ended: wire
                .call
                .as_ref()
                .and_then(|call| instant(call.ended_timestamp.as_deref())),
        },
        4 => SystemEvent::Renamed(wire.content.clone().unwrap_or_default()),
        6 => SystemEvent::Pinned,
        7 => SystemEvent::Joined,
        8..=11 => SystemEvent::Boosted,
        18 => SystemEvent::ThreadCreated(wire.content.clone().unwrap_or_default()),
        other => SystemEvent::Other(system_name(other).to_owned()),
    };
    MessageKind::System(event)
}

fn system_name(kind: u8) -> &'static str {
    match kind {
        5 => "icon_changed",
        12 => "channel_followed",
        14..=17 => "discovery",
        22 => "invite_reminder",
        24 => "auto_moderation",
        25 => "role_subscription",
        27 => "stage_start",
        28 => "stage_end",
        29 => "stage_speaker",
        31 => "stage_topic",
        36..=39 => "incident",
        46 => "poll_result",
        _ => "unknown",
    }
}

fn reply(wire: &wire::Message) -> Option<Reply> {
    let reference = wire
        .message_reference
        .as_ref()
        .filter(|reference| reference.kind != FORWARD)?;
    let message = reference.message_id.as_deref()?;
    let quoted = wire.referenced_message.as_deref().and_then(|referenced| {
        let author = referenced.author.as_ref()?;
        Some(Arc::new(Quoted {
            author: user::user(author),
            excerpt: discord_markdown::parse(referenced.content.as_deref().unwrap_or_default()),
            has_attachments: !referenced.attachments.is_empty()
                || !referenced.sticker_items.is_empty(),
            mentions_author: wire
                .mentions
                .iter()
                .any(|mentioned| mentioned.id == author.id),
        }))
    });
    Some(Reply {
        message: snowflake::id::<MessageId>(message),
        channel: snowflake::id::<ChannelId>(
            reference.channel_id.as_deref().unwrap_or(&wire.channel_id),
        ),
        quoted,
    })
}

fn attachment(wire: &wire::Attachment) -> Attachment {
    let mime = wire.content_type.clone();
    let major = mime
        .as_deref()
        .and_then(|mime| mime.split('/').next())
        .unwrap_or_default();
    let dimensions = wire
        .width
        .zip(wire.height)
        .filter(|&(width, height)| width > 0 && height > 0);
    let kind = match (major, dimensions) {
        ("image", Some((width, height))) => AttachmentKind::Image { width, height },
        ("video", Some((width, height))) => AttachmentKind::Video { width, height },
        ("audio", _) => AttachmentKind::Audio {
            millis: wire.duration_secs.map(|seconds| (seconds * 1000.0) as u64),
        },
        _ => AttachmentKind::File { mime },
    };
    let proxy = wire.proxy_url.as_deref().unwrap_or(&wire.url);
    let preview = match kind {
        AttachmentKind::Image { width, height } => Some(cdn::preview(proxy, width, height, false)),
        AttachmentKind::Video { width, height } => Some(cdn::preview(proxy, width, height, true)),
        _ => None,
    };
    Attachment {
        id: wire.id.clone(),
        name: wire.filename.clone(),
        size: wire.size,
        url: wire.url.clone(),
        preview,
        kind,
        description: wire.description.clone().filter(|text| !text.is_empty()),
        spoiler: wire.filename.starts_with(SPOILER_PREFIX) || wire.flags & SPOILER_FLAG != 0,
    }
}

pub fn embeds(wires: &[wire::Embed]) -> Vec<Embed> {
    wires.iter().map(embed).collect()
}

fn embed(wire: &wire::Embed) -> Embed {
    let kind = match wire.kind.as_deref() {
        Some("image") => EmbedKind::Image,
        Some("video") => EmbedKind::Video,
        Some("gifv") => EmbedKind::Gif,
        Some("link" | "article") => EmbedKind::Link,
        _ => EmbedKind::Rich,
    };
    Embed {
        kind,
        url: wire.url.clone(),
        color: wire.color,
        provider: wire
            .provider
            .as_ref()
            .and_then(|provider| provider.name.clone()),
        author: wire.author.as_ref().map(|author| EmbedLine {
            text: author.name.clone(),
            url: author.url.clone(),
            icon: icon(
                author
                    .proxy_icon_url
                    .as_deref()
                    .or(author.icon_url.as_deref()),
            ),
        }),
        title: wire.title.clone(),
        description: discord_markdown::parse(wire.description.as_deref().unwrap_or_default()),
        fields: wire
            .fields
            .iter()
            .map(|field| EmbedField {
                name: discord_markdown::parse_line(&field.name),
                value: discord_markdown::parse(&field.value),
                inline: field.inline,
            })
            .collect(),
        image: wire.image.as_ref().and_then(media),
        thumbnail: wire.thumbnail.as_ref().and_then(media),
        video: wire.video.as_ref().and_then(media),
        footer: wire.footer.as_ref().map(|footer| EmbedLine {
            text: footer.text.clone(),
            url: None,
            icon: icon(
                footer
                    .proxy_icon_url
                    .as_deref()
                    .or(footer.icon_url.as_deref()),
            ),
        }),
        timestamp: instant(wire.timestamp.as_deref()),
    }
}

fn icon(url: Option<&str>) -> Option<Picture> {
    url.filter(|url| !url.is_empty()).map(Picture::fixed)
}

fn media(wire: &wire::EmbedMedia) -> Option<EmbedMedia> {
    let url = wire.url.clone()?;
    let shown = wire.proxy_url.clone().unwrap_or_else(|| url.clone());
    Some(EmbedMedia {
        picture: Picture::fixed(shown),
        url,
        width: wire.width.unwrap_or(0),
        height: wire.height.unwrap_or(0),
    })
}
