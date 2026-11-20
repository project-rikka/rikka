use serde::Deserialize;

use crate::flex;
use crate::guild::Member;
use crate::user::User;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub user: User,
    pub user_profile: Option<About>,
    pub guild_member: Option<Member>,
    pub guild_member_profile: Option<About>,
    #[serde(deserialize_with = "flex::list")]
    pub badges: Vec<Badge>,
    #[serde(deserialize_with = "flex::list")]
    pub guild_badges: Vec<Badge>,
    #[serde(deserialize_with = "flex::list")]
    pub mutual_guilds: Vec<MutualGuild>,
    #[serde(deserialize_with = "flex::list")]
    pub mutual_friends: Vec<User>,
    #[serde(deserialize_with = "flex::list")]
    pub connected_accounts: Vec<Connected>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct About {
    pub bio: Option<String>,
    pub pronouns: Option<String>,
    pub accent_color: Option<u32>,
    pub banner: Option<String>,
    #[serde(deserialize_with = "flex::list")]
    pub theme_colors: Vec<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Badge {
    pub id: String,
    pub description: String,
    pub icon: String,
    pub link: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MutualGuild {
    pub id: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Connected {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub name: String,
    #[serde(deserialize_with = "flex::flag")]
    pub verified: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Note {
    pub note: Option<String>,
}
