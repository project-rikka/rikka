use discord_core::{
    Abilities, Emoji, ForumSettings, ForumTag, PostLayout, PostSort, TagId, TagMatch,
};
use discord_translate::{Looks, channel};
use discord_wire::Channel as WireChannel;

const LOOKS: Looks = Looks {
    abilities: Abilities::ALL,
    muted: false,
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
