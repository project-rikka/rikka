use discord_core::{
    Abilities, ChannelNotify, Emoji, ForumSettings, ForumTag, Grant, OverwriteTarget, Permissions,
    PostLayout, PostSort, RoleId, SpaceId, TagId, TagMatch, UserId,
};
use discord_translate::{Looks, channel, channel_settings};
use discord_wire::Channel as WireChannel;

const LOOKS: Looks = Looks {
    abilities: Abilities::ALL,
    permissions: Permissions::ALL,
    muted: false,
    notify: ChannelNotify {
        level: None,
        resolved: discord_core::NotifyLevel::All,
        mute_ends: None,
    },
    joined: false,
};

fn wire(json: &str) -> WireChannel {
    serde_json::from_str(json).unwrap()
}

#[test]
fn forum_carries_post_settings() {
    let payload = wire(
        r#"{
            "id": "30", "type": 15, "guild_id": "1", "name": "suggestion",
            "topic": "Be **kind**", "flags": 16,
            "default_sort_order": 1, "default_forum_layout": 2,
            "default_tag_setting": "match_all",
            "default_reaction_emoji": {"emoji_id": null, "emoji_name": "👍"},
            "available_tags": [
                {"id": "5", "name": "Planned", "emoji_id": null, "emoji_name": "📌", "moderated": true}
            ]
        }"#,
    );
    let forum = channel(&payload, Some("1"), LOOKS).unwrap();
    assert_eq!(forum.topic, Some(discord_markdown::parse("Be **kind**")));
    assert_eq!(
        forum.forum,
        Some(ForumSettings {
            tags: vec![ForumTag {
                id: TagId::new(5),
                name: "Planned".to_owned(),
                emoji: Some(Emoji::unicode("📌")),
                moderated: true,
            }],
            sort: PostSort::Created,
            layout: PostLayout::Gallery,
            tag_match: TagMatch::All,
            requires_tag: true,
            reaction: Some(Emoji::unicode("👍")),
        })
    );
}

#[test]
fn unset_forum_settings_follow_discord_defaults() {
    for (kind, layout) in [(15, PostLayout::List), (16, PostLayout::Gallery)] {
        let payload = wire(&format!(
            r#"{{"id": "30", "type": {kind}, "default_sort_order": null, "default_forum_layout": 0}}"#
        ));
        let forum = channel(&payload, Some("1"), LOOKS).unwrap();
        assert_eq!(
            forum.forum,
            Some(ForumSettings {
                layout,
                ..ForumSettings::default()
            }),
            "channel type {kind}"
        );
    }
}

#[test]
fn pinned_flag_marks_forum_post() {
    let payload = wire(
        r#"{
            "id": "40", "type": 11, "guild_id": "1", "parent_id": "30", "name": "Read me",
            "flags": 2, "applied_tags": ["5"],
            "thread_metadata": {"archived": false, "locked": true}
        }"#,
    );
    let post = channel(&payload, Some("1"), LOOKS).unwrap();
    let thread = post.thread.unwrap();
    assert!(thread.pinned);
    assert!(thread.locked);
    assert_eq!(thread.applied_tags, vec![TagId::new(5)]);
    assert_eq!(post.forum, None);
}

#[test]
fn channel_settings_read_overwrites_and_privacy() {
    let payload = wire(
        r#"{
            "id": "40", "type": 2, "guild_id": "1", "name": "lounge",
            "bitrate": 64000, "user_limit": 5, "rtc_region": "japan", "video_quality_mode": 2,
            "permission_overwrites": [
                {"id": "1", "type": 0, "allow": "0", "deny": "1024"},
                {"id": "9", "type": 1, "allow": "1049600", "deny": "0"}
            ]
        }"#,
    );
    let settings = channel_settings(&payload).unwrap();
    assert_eq!(settings.space, SpaceId::new(1));
    assert_eq!(settings.bitrate, 64_000);
    assert_eq!(settings.region.as_deref(), Some("japan"));
    assert!(settings.is_private());
    let member = settings
        .overwrite(OverwriteTarget::Member(UserId::new(9)))
        .copied()
        .unwrap();
    assert_eq!(member.state(Permissions::CONNECT), Grant::Allow);
    assert_eq!(member.state(Permissions::SPEAK), Grant::Inherit);
    let everyone = settings
        .overwrite(OverwriteTarget::Role(RoleId::new(1)))
        .copied()
        .unwrap();
    let opened = everyone.with(Permissions::VIEW_CHANNEL, Grant::Inherit);
    assert!(opened.deny.is_empty());
    let denied = opened
        .with(Permissions::SPEAK, Grant::Allow)
        .with(Permissions::SPEAK, Grant::Deny);
    assert_eq!(denied.state(Permissions::SPEAK), Grant::Deny);
    assert!(!denied.allow.contains(Permissions::SPEAK));
}
