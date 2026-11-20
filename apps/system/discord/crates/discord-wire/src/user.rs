use serde::Deserialize;

use crate::flex;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct User {
    pub id: String,
    pub username: Option<String>,
    pub global_name: Option<String>,
    #[serde(deserialize_with = "flex::text")]
    pub discriminator: Option<String>,
    pub avatar: Option<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub bot: bool,
    pub premium_type: Option<u8>,
    pub banner: Option<String>,
    pub accent_color: Option<u32>,
    pub bio: Option<String>,
    pub pronouns: Option<String>,
}

impl User {
    pub fn is_full(&self) -> bool {
        self.username.is_some()
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Relationship {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: u8,
    pub user: Option<User>,
    pub user_id: Option<String>,
    pub nickname: Option<String>,
}

impl Relationship {
    pub fn user_id(&self) -> &str {
        self.user
            .as_ref()
            .map(|user| user.id.as_str())
            .or(self.user_id.as_deref())
            .unwrap_or(&self.id)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Presence {
    pub user: Option<User>,
    pub user_id: Option<String>,
    pub guild_id: Option<String>,
    pub status: Option<String>,
    #[serde(deserialize_with = "flex::list")]
    pub activities: Vec<Activity>,
    pub client_status: Option<ClientStatus>,
}

impl Presence {
    pub fn user_id(&self) -> Option<&str> {
        self.user
            .as_ref()
            .map(|user| user.id.as_str())
            .or(self.user_id.as_deref())
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Activity {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub kind: u8,
    pub state: Option<String>,
    pub details: Option<String>,
    pub emoji: Option<crate::guild::Emoji>,
    #[serde(deserialize_with = "flex::text")]
    pub application_id: Option<String>,
    pub assets: Option<ActivityAssets>,
    pub timestamps: Option<ActivityTimes>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ActivityAssets {
    pub large_image: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ActivityTimes {
    #[serde(deserialize_with = "flex::text")]
    pub start: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ClientStatus {
    pub desktop: Option<String>,
    pub mobile: Option<String>,
    pub web: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Session {
    pub session_id: String,
    pub status: Option<String>,
    #[serde(deserialize_with = "flex::list")]
    pub activities: Vec<Activity>,
}
