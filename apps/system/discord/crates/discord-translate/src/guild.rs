use discord_core::{
    Emoji, EmojiId, Permissions, Role, RoleId, Space, SpaceId, SpaceNotify, Sticker, StickerFormat,
    StickerId, UserId,
};

use crate::cdn;
use discord_wire as wire;
use discord_wire::snowflake;

#[derive(Clone, Copy, Debug)]
pub struct SpaceLooks {
    pub muted: bool,
    pub unavailable: bool,
    pub permissions: Permissions,
    pub notify: SpaceNotify,
}

pub fn space(id: &str, properties: &wire::Properties, looks: SpaceLooks) -> Space {
    Space {
        id: snowflake::id::<SpaceId>(id),
        name: properties.name.clone().unwrap_or_default(),
        icon: properties
            .icon
            .as_deref()
            .map(|hash| cdn::guild_icon(id, hash)),
        banner: properties
            .banner
            .as_deref()
            .map(|hash| cdn::guild_banner(id, hash)),
        owner: properties.owner_id.as_deref().map(snowflake::id::<UserId>),
        muted: looks.muted,
        unavailable: looks.unavailable,
        permissions: looks.permissions,
        notify: looks.notify,
        features: properties.features.clone(),
        premium_tier: properties.premium_tier.unwrap_or(0),
    }
}

pub fn role(wire: &wire::Role) -> Role {
    Role {
        id: snowflake::id::<RoleId>(wire.id.as_str()),
        name: wire.name.clone(),
        color: wire.colour(),
        position: wire.position,
        hoisted: wire.hoist,
        icon: wire
            .icon
            .as_deref()
            .map(|hash| cdn::role_icon(&wire.id, hash)),
        emoji: wire.unicode_emoji.clone().filter(|emoji| !emoji.is_empty()),
        permissions: Permissions::from_bits(wire.permissions),
        mentionable: wire.mentionable,
        managed: wire.managed,
    }
}

pub fn custom_emoji(wire: &wire::Emoji) -> Option<Emoji> {
    let id = wire.id.as_deref()?;
    Some(Emoji::custom(
        snowflake::id::<EmojiId>(id),
        wire.name.clone().unwrap_or_default(),
        wire.animated,
    ))
}

pub fn reaction_emoji(wire: &wire::Emoji) -> Emoji {
    custom_emoji(wire).unwrap_or_else(|| Emoji::unicode(wire.name.clone().unwrap_or_default()))
}

pub fn sticker(id: &str, name: &str, format_type: u8, description: Option<&str>) -> Sticker {
    let format = match format_type {
        2 => StickerFormat::Apng,
        3 => StickerFormat::Lottie,
        4 => StickerFormat::Gif,
        _ => StickerFormat::Png,
    };
    Sticker {
        id: snowflake::id::<StickerId>(id),
        name: name.to_owned(),
        format,
        picture: Some(cdn::sticker(id, format)),
        description: description
            .map(str::to_owned)
            .filter(|text| !text.is_empty()),
    }
}
