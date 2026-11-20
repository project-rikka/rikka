use std::collections::BTreeMap;
use std::time::Instant;

use discord_core::{
    Abilities, ChannelId, Focus, Ready, RosterGroup, RosterRow, SpaceId, Update, UserId,
};
use discord_state::{Moment, Outcome, SUPPLEMENT_WAIT, Signal, State};
use serde_json::{Value, json};

const START: i64 = 1_700_000_000_000;
const VIEW: u64 = 1 << 10;
const SEND: u64 = 1 << 11;
const READ: u64 = 1 << 16;
const ADMINISTRATOR: u64 = 1 << 3;

fn at(unix_ms: i64) -> Moment {
    Moment {
        unix_ms,
        instant: Instant::now(),
    }
}

fn guild(everyone: u64, overwrites: Value, member: Value) -> Value {
    json!({
        "id": "10",
        "name": "Home",
        "owner_id": "1",
        "roles": [
            {"id": "10", "permissions": everyone.to_string()},
            {"id": "11", "permissions": "0"}
        ],
        "channels": [
            {"id": "20", "type": 0, "guild_id": "10", "name": "general",
             "permission_overwrites": overwrites}
        ],
        "members": [member]
    })
}

fn member(roles: &[&str]) -> Value {
    json!({"user": {"id": "100", "username": "me"}, "roles": roles})
}

fn ready_payload(guild: &Value) -> String {
    json!({
        "user": {"id": "100", "username": "me"},
        "session_id": "session",
        "guilds": [guild],
        "private_channels": [{"id": "30", "type": 1, "recipient_ids": ["7"]}],
        "read_state": {"entries": [], "partial": false}
    })
    .to_string()
}

fn ready(outcome: &Outcome) -> Option<&Ready> {
    outcome.updates.iter().find_map(|update| match update {
        Update::Ready(ready) => Some(&**ready),
        _ => None,
    })
}

fn signed_in(guild: &Value) -> (State, Ready) {
    let mut state = State::new(at(START));
    assert!(
        state
            .dispatch("READY", &ready_payload(guild), at(START))
            .updates
            .is_empty()
    );
    let outcome = state.dispatch("READY_SUPPLEMENTAL", "{}", at(START));
    let ready = ready(&outcome).cloned().unwrap();
    (state, ready)
}

fn abilities(ready: &Ready, channel: u64) -> Abilities {
    ready
        .channels
        .iter()
        .find(|shown| shown.id == ChannelId::new(channel))
        .map(|shown| shown.abilities)
        .unwrap()
}

#[test]
fn ready_waits_for_supplemental() {
    let mut state = State::new(at(START));
    let home = guild(VIEW | SEND | READ, json!([]), member(&[]));
    assert!(
        state
            .dispatch("READY", &ready_payload(&home), at(START))
            .updates
            .is_empty()
    );
    let message = json!({
        "id": "500", "channel_id": "20", "guild_id": "10", "type": 0,
        "author": {"id": "7", "username": "ann"}, "content": "hi"
    })
    .to_string();
    assert!(
        state
            .dispatch("MESSAGE_CREATE", &message, at(START))
            .updates
            .is_empty()
    );
    let outcome = state.dispatch("READY_SUPPLEMENTAL", "{}", at(START));
    let ready = ready(&outcome).unwrap();
    assert_eq!(ready.me.id, UserId::new(100));
    assert_eq!(ready.spaces.len(), 1);
    let mut channels: Vec<u64> = ready.channels.iter().map(|shown| shown.id.get()).collect();
    channels.sort_unstable();
    assert_eq!(channels, [20, 30]);
    assert!(outcome.updates.iter().any(|update| matches!(
        update,
        Update::Message(message) if message.text == "hi"
    )));
}

#[test]
fn late_supplement_releases_ready_after_wait() {
    let mut state = State::new(at(START));
    let home = guild(VIEW, json!([]), member(&[]));
    state.dispatch("READY", &ready_payload(&home), at(START));
    let wait = i64::try_from(SUPPLEMENT_WAIT.as_millis()).unwrap();
    assert!(ready(&state.tick(at(START + wait - 1))).is_none());
    assert!(ready(&state.tick(at(START + wait))).is_some());
}

#[test]
fn channel_abilities_follow_permissions() {
    let deny_send = json!([{"id": "10", "type": 0, "allow": "0", "deny": SEND.to_string()}]);
    let cases = [
        (
            "everyone may send",
            VIEW | SEND | READ,
            json!([]),
            member(&[]),
            true,
            true,
        ),
        (
            "everyone overwrite denies",
            VIEW | SEND | READ,
            deny_send.clone(),
            member(&[]),
            true,
            false,
        ),
        (
            "role overwrite allows again",
            VIEW | SEND | READ,
            json!([
                {"id": "10", "type": 0, "allow": "0", "deny": SEND.to_string()},
                {"id": "11", "type": 0, "allow": SEND.to_string(), "deny": "0"}
            ]),
            member(&["11"]),
            true,
            true,
        ),
        (
            "member overwrite hides",
            VIEW | SEND | READ,
            json!([{"id": "100", "type": 1, "allow": "0", "deny": VIEW.to_string()}]),
            member(&[]),
            false,
            false,
        ),
        (
            "timeout keeps reading only",
            VIEW | SEND | READ,
            json!([]),
            json!({"user": {"id": "100"}, "roles": [],
                   "communication_disabled_until": "2023-11-14T22:14:20+00:00"}),
            true,
            false,
        ),
        (
            "past timeout is lifted",
            VIEW | SEND | READ,
            json!([]),
            json!({"user": {"id": "100"}, "roles": [],
                   "communication_disabled_until": "2023-11-14T22:13:19+00:00"}),
            true,
            true,
        ),
        (
            "administrator ignores overwrites",
            ADMINISTRATOR,
            deny_send,
            member(&[]),
            true,
            true,
        ),
    ];
    for (name, everyone, overwrites, mine, view, send) in cases {
        let (_, ready) = signed_in(&guild(everyone, overwrites, mine));
        let channel = abilities(&ready, 20);
        assert_eq!(channel.contains(Abilities::VIEW), view, "{name}");
        assert_eq!(channel.contains(Abilities::SEND), send, "{name}");
    }
    let (_, ready) = signed_in(&guild(VIEW, json!([]), member(&[])));
    assert!(abilities(&ready, 30).contains(Abilities::SEND));
}

#[test]
fn timeout_ends_on_tick() {
    let timed_out = json!({"user": {"id": "100"}, "roles": [],
        "communication_disabled_until": "2023-11-14T22:14:20+00:00"});
    let (mut state, _) = signed_in(&guild(VIEW | SEND | READ, json!([]), timed_out));
    let sendable = |outcome: &Outcome| {
        outcome.updates.iter().find_map(|update| match update {
            Update::Channel(channel) if channel.id == ChannelId::new(20) => {
                Some(channel.abilities.contains(Abilities::SEND))
            }
            _ => None,
        })
    };
    assert_eq!(sendable(&state.tick(at(START + 30_000))), None);
    assert_eq!(sendable(&state.tick(at(START + 60_000))), Some(true));
}

#[test]
fn typing_carries_dispatch_moment() {
    let (mut state, _) = signed_in(&guild(VIEW, json!([]), member(&[])));
    let moment = at(START + 5);
    let typing = json!({"channel_id": "20", "guild_id": "10", "user_id": "7"}).to_string();
    let outcome = state.dispatch("TYPING_START", &typing, moment);
    assert!(outcome.updates.iter().any(|update| matches!(
        update,
        Update::Typing { channel, user, at } if *channel == ChannelId::new(20)
            && *user == UserId::new(7)
            && *at == moment.instant
    )));
}

#[test]
fn own_voice_events_become_signals() {
    let (mut state, _) = signed_in(&guild(VIEW, json!([]), member(&[])));
    let own = json!({"user_id": "100", "guild_id": "10", "channel_id": "40", "session_id": "abc"});
    let other = json!({"user_id": "7", "guild_id": "10", "channel_id": "40", "session_id": "xyz"});
    let server = json!({"guild_id": "10", "token": "secret", "endpoint": "voice.discord.media"});
    assert_eq!(
        state
            .dispatch("VOICE_STATE_UPDATE", &own.to_string(), at(START))
            .voice,
        [Signal::Session {
            guild: Some("10".to_owned()),
            channel: Some("40".to_owned()),
            session: "abc".to_owned(),
        }]
    );
    assert!(
        state
            .dispatch("VOICE_STATE_UPDATE", &other.to_string(), at(START))
            .voice
            .is_empty()
    );
    assert_eq!(
        state
            .dispatch("VOICE_SERVER_UPDATE", &server.to_string(), at(START))
            .voice,
        [Signal::Server {
            guild: Some("10".to_owned()),
            channel: None,
            token: "secret".to_owned(),
            endpoint: Some("voice.discord.media".to_owned()),
        }]
    );
}

#[test]
fn member_list_follows_focused_channel() {
    let overwrites = json!([
        {"id": "10", "type": 0, "allow": "0", "deny": VIEW.to_string()},
        {"id": "11", "type": 0, "allow": VIEW.to_string(), "deny": "0"}
    ]);
    let (mut state, _) = signed_in(&guild(VIEW | SEND, overwrites, member(&["11"])));
    let focused = state.focus(Focus {
        space: Some(SpaceId::new(10)),
        channel: Some(ChannelId::new(20)),
    });
    assert_eq!(
        focused.commands,
        [json!({"op": 37, "d": {"subscriptions": {"10": {
            "typing": true, "activities": true, "threads": true,
            "channels": {"20": [[0, 99]]}
        }}}})]
    );
    let scrolled = state.view_roster(ChannelId::new(20), 250..260);
    assert_eq!(
        scrolled.commands[0]["d"]["subscriptions"]["10"]["channels"]["20"],
        json!([[0, 99], [200, 299]])
    );

    let list = |id: &str| {
        json!({
            "guild_id": "10",
            "id": id,
            "groups": [{"id": "online", "count": 1}],
            "ops": [{"op": "SYNC", "range": [0, 99], "items": [
                {"group": {"id": "online", "count": 1}},
                {"member": {"user": {"id": "7", "username": "ann"}, "roles": []}}
            ]}]
        })
        .to_string()
    };
    let roster = |outcome: &Outcome| {
        outcome.updates.iter().find_map(|update| match update {
            Update::Roster { channel, roster } if *channel == ChannelId::new(20) => {
                Some(roster.clone())
            }
            _ => None,
        })
    };
    let other = state.dispatch("GUILD_MEMBER_LIST_UPDATE", &list("everyone"), at(START));
    assert!(roster(&other).is_none());
    let matching = state.dispatch("GUILD_MEMBER_LIST_UPDATE", &list("3462558076"), at(START));
    let shown = roster(&matching).unwrap();
    assert_eq!(shown.len, 2);
    assert_eq!(
        shown.known,
        BTreeMap::from([
            (
                0,
                RosterRow::Group {
                    label: RosterGroup::Online,
                    count: 1
                }
            ),
            (1, RosterRow::Member(UserId::new(7))),
        ])
    );
}
