use discord_client::writing::{emoji, mention, message_link};
use discord_core::{
    Abilities, Channel, ChannelId, ChannelKind, Emoji, EmojiId, Mention, MessageId, Picture,
    RoleId, SpaceId, UserId,
};

fn channel(space: Option<u64>) -> Channel {
    Channel {
        id: ChannelId::new(20),
        space: space.map(SpaceId::new),
        kind: ChannelKind::Text,
        name: String::new(),
        topic: None,
        parent: None,
        position: 0,
        icon: None,
        recipients: Vec::new(),
        abilities: Abilities::ALL,
        nsfw: false,
        muted: false,
        slowmode: 0,
        last_activity: None,
        thread: None,
        forum: None,
        bitrate: None,
        user_limit: None,
    }
}

#[test]
fn mentions_use_discord_markup() {
    assert_eq!(mention(&Mention::User(UserId::new(1))), "<@1>");
    assert_eq!(mention(&Mention::Role(RoleId::new(2))), "<@&2>");
    assert_eq!(mention(&Mention::Channel(ChannelId::new(3))), "<#3>");
    assert_eq!(mention(&Mention::Everyone), "@everyone");
}

#[test]
fn custom_emoji_keep_id_and_animation() {
    let still = Emoji {
        id: Some(EmojiId::new(9)),
        name: "wave".into(),
        picture: Some(Picture::fixed("https://cdn/9.webp")),
    };
    let moving = Emoji {
        picture: Some(Picture::fixed("https://cdn/9.webp").animated(true)),
        ..still.clone()
    };

    assert_eq!(emoji(&still), "<:wave:9>");
    assert_eq!(emoji(&moving), "<a:wave:9>");
    assert_eq!(emoji(&Emoji::unicode("👍")), "👍");
}

#[test]
fn message_link_names_guild_or_me() {
    assert_eq!(
        message_link(&channel(Some(10)), &MessageId::new(30)),
        "https://discord.com/channels/10/20/30"
    );
    assert_eq!(
        message_link(&channel(None), &MessageId::new(30)),
        "https://discord.com/channels/@me/20/30"
    );
}
