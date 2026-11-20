use discord_core::{Status, StreamKey};
use serde::Serialize;
use serde_json::{Value, json};

use crate::preloaded::status_name;

const CAPABILITIES: u32 = (1 << 0)
    | (1 << 2)
    | (1 << 3)
    | (1 << 4)
    | (1 << 5)
    | (1 << 6)
    | (1 << 7)
    | (1 << 8)
    | (1 << 9)
    | (1 << 10)
    | (1 << 12)
    | (1 << 13)
    | (1 << 14)
    | (1 << 17)
    | (1 << 19)
    | (1 << 20);
const QOS_VERSION: u32 = 30;
const MEMBERS_PER_REQUEST: usize = 100;

pub fn identify(token: &str, properties: &impl Serialize) -> Value {
    json!({
        "op": 2,
        "d": {
            "token": token,
            "capabilities": CAPABILITIES,
            "properties": properties,
            "presence": {
                "status": "unknown",
                "since": 0,
                "activities": [],
                "afk": false,
            },
            "compress": false,
            "client_state": { "guild_versions": {} },
        }
    })
}

pub fn resume(token: &str, session: &str, sequence: Option<u64>) -> Value {
    json!({
        "op": 6,
        "d": { "token": token, "session_id": session, "seq": sequence }
    })
}

pub fn heartbeat(sequence: Option<u64>) -> Value {
    json!({
        "op": 40,
        "d": {
            "seq": sequence,
            "qos": { "active": true, "ver": QOS_VERSION, "reasons": ["foregrounded"] },
        }
    })
}

pub fn subscribe(guild: &str, channel: &str, ranges: &[[u32; 2]]) -> Value {
    json!({
        "op": 37,
        "d": {
            "subscriptions": {
                guild: {
                    "typing": true,
                    "activities": true,
                    "threads": true,
                    "channels": { channel: ranges },
                }
            }
        }
    })
}

pub fn request_members(guild: &str, users: &[String]) -> Vec<Value> {
    users
        .chunks(MEMBERS_PER_REQUEST)
        .map(|chunk| {
            json!({
                "op": 8,
                "d": { "guild_id": [guild], "user_ids": chunk, "presences": false }
            })
        })
        .collect()
}

pub fn voice_state(
    guild: Option<&str>,
    channel: Option<&str>,
    muted: bool,
    deafened: bool,
) -> Value {
    json!({
        "op": 4,
        "d": {
            "guild_id": guild,
            "channel_id": channel,
            "self_mute": muted,
            "self_deaf": deafened,
            "self_video": false,
        }
    })
}

pub fn stream_create(guild: Option<&str>, channel: &str) -> Value {
    json!({
        "op": 18,
        "d": {
            "type": if guild.is_some() { "guild" } else { "call" },
            "guild_id": guild,
            "channel_id": channel,
            "preferred_region": null,
        }
    })
}

pub fn stream_paused(key: StreamKey, paused: bool) -> Value {
    json!({ "op": 22, "d": { "stream_key": key.to_string(), "paused": paused } })
}

pub fn stream_delete(key: StreamKey) -> Value {
    json!({ "op": 19, "d": { "stream_key": key.to_string() } })
}

pub fn stream_watch(key: StreamKey) -> Value {
    json!({ "op": 20, "d": { "stream_key": key.to_string() } })
}

pub fn presence(status: Status, activities: Vec<Value>) -> Value {
    json!({
        "op": 3,
        "d": { "status": status_name(status), "since": 0, "activities": activities, "afk": false }
    })
}
