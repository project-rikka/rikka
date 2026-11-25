use discord_core::{
    Activity, ActivityKind, Bond, Friend, Member, Presence, SpaceId, Status, User, UserId,
};

use super::{filled, instant};
use crate::cdn;
use discord_wire as wire;
use discord_wire::snowflake;
use discord_wire::status_named;

pub fn user(wire: &wire::User) -> User {
    let username = wire.username.clone().unwrap_or_default();
    let legacy = wire
        .discriminator
        .as_deref()
        .filter(|tag| !tag.is_empty() && *tag != "0");
    let handle = match legacy {
        Some(tag) => format!("{username}#{tag}"),
        None => username,
    };
    User {
        id: snowflake::id::<UserId>(wire.id.as_str()),
        handle,
        display: wire.global_name.clone().filter(|name| !name.is_empty()),
        avatar: Some(cdn::avatar(
            &wire.id,
            wire.avatar.as_deref(),
            wire.discriminator.as_deref(),
        )),
        bot: wire.bot,
    }
}

pub fn member(guild: &str, wire: &wire::Member) -> Option<Member> {
    let user = wire.user_id()?;
    Some(Member {
        user: snowflake::id::<UserId>(user),
        space: snowflake::id::<SpaceId>(guild),
        nickname: wire.nick.clone().filter(|nick| !nick.is_empty()),
        avatar: wire
            .avatar
            .as_deref()
            .map(|hash| cdn::member_avatar(guild, user, hash)),
        roles: wire.roles.iter().map(|role| snowflake::id(role)).collect(),
        joined: instant(wire.joined_at.as_deref()),
        timeout_until: instant(wire.communication_disabled_until.as_deref()),
    })
}

pub fn presence(
    status: Option<&str>,
    activities: &[wire::Activity],
    client: Option<&wire::ClientStatus>,
) -> Presence {
    let mood = activities
        .iter()
        .find(|activity| activity.kind == 4)
        .and_then(|custom| {
            let emoji = custom
                .emoji
                .as_ref()
                .filter(|emoji| emoji.id.is_none())
                .and_then(|emoji| emoji.name.clone());
            let text = custom.state.clone().filter(|state| !state.is_empty());
            match (emoji, text) {
                (Some(emoji), Some(text)) => Some(format!("{emoji} {text}")),
                (emoji, text) => text.or(emoji),
            }
        });
    let activity = activities.iter().find_map(|activity| {
        let kind = match activity.kind {
            0 => ActivityKind::Playing,
            1 => ActivityKind::Streaming,
            2 => ActivityKind::Listening,
            3 => ActivityKind::Watching,
            5 => ActivityKind::Competing,
            _ => return None,
        };
        Some(Activity {
            kind,
            name: activity.name.clone().unwrap_or_default(),
            details: filled(activity.details.as_deref()).map(str::to_owned),
            state: filled(activity.state.as_deref()).map(str::to_owned),
            art: activity
                .assets
                .as_ref()
                .and_then(|assets| assets.large_image.as_deref())
                .and_then(|asset| cdn::activity_art(activity.application_id.as_deref(), asset)),
            started: activity
                .timestamps
                .as_ref()
                .and_then(|timestamps| timestamps.start.as_deref())
                .and_then(|start| start.parse().ok()),
        })
    });
    let mobile = client.is_some_and(|client| {
        client.mobile.is_some() && client.desktop.is_none() && client.web.is_none()
    });
    Presence {
        status: status.map_or(Status::Offline, status_named),
        mood,
        activity,
        mobile,
    }
}

pub fn friend(wire: &wire::Relationship) -> Option<Friend> {
    let bond = match wire.kind {
        1 => Bond::Friend,
        2 => Bond::Blocked,
        3 => Bond::Incoming,
        4 => Bond::Outgoing,
        _ => return None,
    };
    Some(Friend {
        user: snowflake::id::<UserId>(wire.user_id()),
        bond,
        nickname: wire
            .nickname
            .clone()
            .filter(|nickname| !nickname.is_empty()),
    })
}
