use discord_core::{ChannelId, SpaceId, Status, StreamKey, UserId};
use discord_wire::commands;
use serde_json::{Value, json};

#[test]
fn commands_match_gateway_payloads() {
    let key = StreamKey {
        space: Some(SpaceId::new(1)),
        channel: ChannelId::new(2),
        owner: UserId::new(3),
    };
    let cases: Vec<(&str, Value, Value)> = vec![
        (
            "resume",
            commands::resume("token", "session", Some(42)),
            json!({ "op": 6, "d": { "token": "token", "session_id": "session", "seq": 42 } }),
        ),
        (
            "subscribe",
            commands::subscribe("10", "20", &[[0, 99], [100, 199]]),
            json!({ "op": 37, "d": { "subscriptions": { "10": {
                "typing": true,
                "activities": true,
                "threads": true,
                "channels": { "20": [[0, 99], [100, 199]] },
            } } } }),
        ),
        (
            "voice state",
            commands::voice_state(Some("10"), Some("20"), true, false),
            json!({ "op": 4, "d": {
                "guild_id": "10",
                "channel_id": "20",
                "self_mute": true,
                "self_deaf": false,
                "self_video": false,
            } }),
        ),
        (
            "leave voice",
            commands::voice_state(None, None, false, false),
            json!({ "op": 4, "d": {
                "guild_id": null,
                "channel_id": null,
                "self_mute": false,
                "self_deaf": false,
                "self_video": false,
            } }),
        ),
        (
            "stream in guild",
            commands::stream_create(Some("10"), "20"),
            json!({ "op": 18, "d": {
                "type": "guild",
                "guild_id": "10",
                "channel_id": "20",
                "preferred_region": null,
            } }),
        ),
        (
            "stream in call",
            commands::stream_create(None, "20"),
            json!({ "op": 18, "d": {
                "type": "call",
                "guild_id": null,
                "channel_id": "20",
                "preferred_region": null,
            } }),
        ),
        (
            "stream paused",
            commands::stream_paused(key, true),
            json!({ "op": 22, "d": { "stream_key": "guild:1:2:3", "paused": true } }),
        ),
        (
            "stream delete",
            commands::stream_delete(key),
            json!({ "op": 19, "d": { "stream_key": "guild:1:2:3" } }),
        ),
        (
            "stream watch",
            commands::stream_watch(key),
            json!({ "op": 20, "d": { "stream_key": "guild:1:2:3" } }),
        ),
        (
            "presence",
            commands::presence(Status::Busy, vec![json!({ "type": 4 })]),
            json!({ "op": 3, "d": {
                "status": "dnd",
                "since": 0,
                "activities": [{ "type": 4 }],
                "afk": false,
            } }),
        ),
    ];
    for (name, command, expected) in cases {
        assert_eq!(command, expected, "{name}");
    }
}

#[test]
fn heartbeat_carries_sequence_and_qos() {
    let beat = commands::heartbeat(None);
    assert_eq!(beat["op"], 40);
    assert_eq!(beat["d"]["seq"], Value::Null);
    assert_eq!(beat["d"]["qos"]["active"], true);
    assert_eq!(commands::heartbeat(Some(7))["d"]["seq"], 7);
}

#[test]
fn identify_carries_token_and_properties() {
    let identify = commands::identify("token", &json!({ "os": "Windows" }));
    assert_eq!(identify["op"], 2);
    assert_eq!(identify["d"]["token"], "token");
    assert_eq!(identify["d"]["properties"], json!({ "os": "Windows" }));
    assert_eq!(identify["d"]["compress"], false);
}

#[test]
fn member_requests_are_chunked_by_hundred() {
    let users: Vec<String> = (0..250).map(|user| user.to_string()).collect();
    let requests = commands::request_members("10", &users);
    let sizes: Vec<usize> = requests
        .iter()
        .map(|request| request["d"]["user_ids"].as_array().map_or(0, Vec::len))
        .collect();
    assert_eq!(sizes, [100, 100, 50]);
    assert!(
        requests
            .iter()
            .all(|request| request["op"] == 8 && request["d"]["guild_id"] == json!(["10"]))
    );
}
