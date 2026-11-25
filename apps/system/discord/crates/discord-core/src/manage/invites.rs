use crate::id::{ChannelId, IntegrationId, WebhookId};
use crate::model::picture::Picture;
use crate::model::user::User;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceInvite {
    pub code: String,
    pub inviter: Option<User>,
    pub channel: Option<ChannelId>,
    pub channel_name: Option<String>,
    pub uses: u32,
    pub max_uses: u32,
    pub max_age: u32,
    pub temporary: bool,
    pub expires: Option<i64>,
}

impl SpaceInvite {
    #[must_use]
    pub fn url(&self) -> String {
        invite_url(&self.code)
    }
}

#[must_use]
pub fn invite_url(code: &str) -> String {
    format!("https://discord.gg/{code}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InviteAge {
    Minutes30,
    Hour,
    Hours6,
    Hours12,
    Day,
    Week,
    Never,
}

impl InviteAge {
    pub const ALL: [InviteAge; 7] = [
        InviteAge::Minutes30,
        InviteAge::Hour,
        InviteAge::Hours6,
        InviteAge::Hours12,
        InviteAge::Day,
        InviteAge::Week,
        InviteAge::Never,
    ];

    #[must_use]
    pub const fn seconds(self) -> u32 {
        match self {
            InviteAge::Minutes30 => 1800,
            InviteAge::Hour => 3600,
            InviteAge::Hours6 => 6 * 3600,
            InviteAge::Hours12 => 12 * 3600,
            InviteAge::Day => 86_400,
            InviteAge::Week => 604_800,
            InviteAge::Never => 0,
        }
    }
}

pub const INVITE_USES: [u32; 7] = [0, 1, 5, 10, 25, 50, 100];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InviteRequest {
    pub age: InviteAge,
    pub max_uses: u32,
    pub temporary: bool,
}

impl Default for InviteRequest {
    fn default() -> InviteRequest {
        InviteRequest {
            age: InviteAge::Week,
            max_uses: 0,
            temporary: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Webhook {
    pub id: WebhookId,
    pub name: String,
    pub avatar: Option<Picture>,
    pub channel: Option<ChannelId>,
    pub creator: Option<User>,
    pub url: Option<String>,
    pub incoming: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Integration {
    pub id: IntegrationId,
    pub name: String,
    pub kind: String,
    pub enabled: bool,
    pub account: Option<String>,
    pub application: Option<String>,
    pub icon: Option<Picture>,
    pub creator: Option<User>,
}
