use discord_core::{Badge, Connected, Profile, Rich, SpaceId, SpaceProfile, User};

use super::{filled, instant, user};
use crate::cdn;
use discord_wire as wire;
use discord_wire::About;
use discord_wire::snowflake;

pub fn profile(wire: &wire::Profile, space: Option<&str>) -> Profile {
    let about = wire.user_profile.clone().unwrap_or_default();
    let local = space
        .and(wire.guild_member_profile.clone())
        .unwrap_or_default();
    let id = wire.user.id.as_str();
    let space_profile = space
        .zip(wire.guild_member.as_ref())
        .map(|(space, member)| SpaceProfile {
            space: snowflake::id::<SpaceId>(space),
            nickname: member.nick.clone().filter(|nick| !nick.is_empty()),
            avatar: member
                .avatar
                .as_deref()
                .map(|hash| cdn::member_avatar(space, id, hash)),
            roles: member
                .roles
                .iter()
                .map(|role| snowflake::id(role))
                .collect(),
            joined: instant(member.joined_at.as_deref()),
        });
    let local_banner = space
        .zip(local.banner.as_deref())
        .map(|(space, hash)| cdn::member_banner(space, id, hash));
    let bio = filled(local.bio.as_deref())
        .or_else(|| filled(about.bio.as_deref()))
        .or_else(|| filled(wire.user.bio.as_deref()));
    Profile {
        user: user::user(&wire.user),
        bio: bio.map_or_else(Rich::default, discord_markdown::parse),
        pronouns: filled(local.pronouns.as_deref())
            .or_else(|| filled(about.pronouns.as_deref()))
            .or_else(|| filled(wire.user.pronouns.as_deref()))
            .map(str::to_owned),
        banner: local_banner.or_else(|| {
            about
                .banner
                .as_deref()
                .or(wire.user.banner.as_deref())
                .map(|hash| cdn::user_banner(id, hash))
        }),
        accent: local
            .accent_color
            .or(about.accent_color)
            .or(wire.user.accent_color),
        theme: theme(&local).or_else(|| theme(&about)),
        badges: wire
            .guild_badges
            .iter()
            .chain(&wire.badges)
            .map(badge)
            .collect(),
        space: space_profile,
        mutual_spaces: wire
            .mutual_guilds
            .iter()
            .map(|guild| snowflake::id::<SpaceId>(guild.id.as_str()))
            .collect(),
        mutual_friends: mutual_friends(&wire.mutual_friends),
        connections: wire.connected_accounts.iter().map(connected).collect(),
    }
}

fn theme(about: &About) -> Option<u32> {
    about.theme_colors.first().copied()
}

fn badge(wire: &wire::Badge) -> Badge {
    Badge {
        id: wire.id.clone(),
        description: wire.description.clone(),
        icon: cdn::badge(&wire.icon),
        link: wire
            .link
            .clone()
            .filter(|link| link.starts_with("https://")),
    }
}

fn mutual_friends(wire: &[wire::User]) -> Vec<User> {
    let mut friends: Vec<User> = wire
        .iter()
        .filter(|friend| friend.is_full())
        .map(user::user)
        .collect();
    friends.sort_by_cached_key(|friend| friend.name().to_lowercase());
    friends
}

fn connected(wire: &wire::Connected) -> Connected {
    let name = &wire.name;
    let id = &wire.id;
    let url = match wire.kind.as_str() {
        "github" => Some(format!("https://github.com/{name}")),
        "twitch" => Some(format!("https://www.twitch.tv/{name}")),
        "youtube" => Some(format!("https://www.youtube.com/channel/{id}")),
        "steam" => Some(format!("https://steamcommunity.com/profiles/{id}")),
        "reddit" => Some(format!("https://www.reddit.com/u/{name}")),
        "spotify" => Some(format!("https://open.spotify.com/user/{id}")),
        "twitter" => Some(format!("https://x.com/{name}")),
        "bluesky" => Some(format!("https://bsky.app/profile/{id}")),
        "tiktok" => Some(format!("https://www.tiktok.com/@{name}")),
        "instagram" => Some(format!("https://www.instagram.com/{name}")),
        "facebook" => Some(format!("https://www.facebook.com/{id}")),
        "roblox" => Some(format!("https://www.roblox.com/users/{id}/profile")),
        "ebay" => Some(format!("https://www.ebay.com/usr/{name}")),
        "domain" => Some(format!("https://{name}")),
        "mastodon" => name
            .trim_start_matches('@')
            .split_once('@')
            .map(|(user, host)| format!("https://{host}/@{user}")),
        _ => None,
    };
    Connected {
        kind: wire.kind.clone(),
        name: name.clone(),
        url,
        verified: wire.verified,
    }
}
