use discord_core::{SpaceId, Status};
use discord_wire::{Channel, Coverage, Message, Preloaded, Ready, snowflake};

#[test]
fn snowflakes_decode_discord_epoch() {
    assert_eq!(
        snowflake::millis("175928847299117063"),
        Some(1_462_015_105_796)
    );
    assert_eq!(snowflake::millis("not a number"), None);
    assert_eq!(snowflake::value("x"), 0);
    assert!(snowflake::newer("175928847299117064", "175928847299117063"));
}

#[test]
fn tolerant_fields_accept_discord_variants() {
    let message: Message = serde_json::from_str(
        r#"{
            "id": "10",
            "channel_id": "20",
            "type": 0,
            "content": "hi",
            "nonce": 1234,
            "mentions": [{"id": "1", "username": "a"}, {"id": 5}],
            "mention_roles": null,
            "pinned": null,
            "attachments": []
        }"#,
    )
    .unwrap();
    assert_eq!(message.nonce.as_deref(), Some("1234"));
    assert_eq!(message.mentions.len(), 1);
    assert!(message.mention_roles.is_empty());
    assert!(!message.pinned);

    let channel: Channel = serde_json::from_str(
        r#"{
            "id": "30",
            "type": 0,
            "last_message_id": 0,
            "applied_tags": [7, "8"],
            "permission_overwrites": [
                {"id": "1", "type": "member", "allow": "1024", "deny": 0},
                {"id": "2", "type": 0, "allow": 8, "deny": "2048"}
            ]
        }"#,
    )
    .unwrap();
    assert_eq!(channel.last_message_id, None);
    assert_eq!(channel.applied_tags, ["7", "8"]);
    let overwrites: Vec<(bool, u64, u64)> = channel
        .permission_overwrites
        .iter()
        .map(|overwrite| (overwrite.is_member(), overwrite.allow, overwrite.deny))
        .collect();
    assert_eq!(overwrites, [(true, 1024, 0), (false, 8, 2048)]);
}

#[test]
fn ready_reads_listed_and_plain_states() {
    let ready: Ready = serde_json::from_str(
        r#"{
            "user": {"id": "1", "username": "me"},
            "session_id": "abc",
            "read_state": {"entries": [{"id": "20", "last_message_id": "5"}], "partial": false},
            "user_guild_settings": [{"guild_id": "10"}]
        }"#,
    )
    .unwrap();
    assert_eq!(ready.read_state.len(), 1);
    assert_eq!(ready.user_guild_settings.len(), 1);
    assert_eq!(ready.session_id, "abc");
}

#[test]
fn settings_patch_round_trips_status() {
    let patch = Preloaded::status_patch(Status::Busy);
    let decoded = Preloaded::decode_base64(&patch);
    assert_eq!(decoded.status(), Some(Status::Busy));
    assert_eq!(Preloaded::decode_base64("not base64").status(), None);
}

#[test]
fn partial_settings_keep_unmentioned_parts() {
    let mut settings = Preloaded {
        guild_folders: Some(discord_wire::GuildFolders {
            folders: vec![discord_wire::GuildFolder {
                guild_ids: vec![10, 11],
                id: Some(discord_wire::Int64Value { value: 3 }),
                name: Some(discord_wire::StringValue {
                    value: "games".to_owned(),
                }),
                color: None,
            }],
        }),
        ..Preloaded::default()
    };
    settings.absorb(
        Preloaded::decode_base64(&Preloaded::status_patch(Status::Idle)),
        Coverage::Partial,
    );
    assert_eq!(settings.status(), Some(Status::Idle));
    let folders = settings.folders();
    assert_eq!(folders.len(), 1);
    assert_eq!(folders[0].spaces, [SpaceId::new(10), SpaceId::new(11)]);
    assert_eq!(folders[0].name.as_deref(), Some("games"));

    settings.absorb(
        Preloaded::decode_base64(&Preloaded::status_patch(Status::Online)),
        Coverage::Complete,
    );
    assert_eq!(settings.status(), Some(Status::Online));
    assert_eq!(settings.folders().len(), 1);
}

#[test]
fn custom_status_expires() {
    let settings = Preloaded {
        status: Some(discord_wire::StatusSettings {
            status: None,
            custom_status: Some(discord_wire::CustomStatus {
                text: "away".to_owned(),
                emoji_id: 0,
                emoji_name: String::new(),
                expires_at_ms: 2_000,
            }),
        }),
        ..Preloaded::default()
    };
    assert_eq!(
        settings
            .custom_status(1_000)
            .map(|custom| custom.text.as_str()),
        Some("away")
    );
    assert!(settings.custom_status(2_000).is_none());
}
