use std::sync::Arc;

use crate::id::{ChannelId, SpaceId, UserId};
use crate::model::notify::NotifyLevel;
use crate::model::picture::Picture;

pub const COMMUNITY: &str = "COMMUNITY";
pub const VANITY_URL: &str = "VANITY_URL";
pub const BANNER: &str = "BANNER";
pub const INVITE_SPLASH: &str = "INVITE_SPLASH";
pub const ROLE_ICONS: &str = "ROLE_ICONS";
pub const WELCOME_SCREEN_ENABLED: &str = "WELCOME_SCREEN_ENABLED";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum VerificationLevel {
    #[default]
    None,
    Low,
    Medium,
    High,
    Highest,
}

impl VerificationLevel {
    pub const ALL: [VerificationLevel; 5] = [
        VerificationLevel::None,
        VerificationLevel::Low,
        VerificationLevel::Medium,
        VerificationLevel::High,
        VerificationLevel::Highest,
    ];

    #[must_use]
    pub fn from_code(code: u8) -> VerificationLevel {
        VerificationLevel::ALL
            .get(usize::from(code))
            .copied()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn code(self) -> u8 {
        VerificationLevel::ALL
            .iter()
            .position(|level| *level == self)
            .map_or(0, |at| at as u8)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ContentFilter {
    #[default]
    Disabled,
    MembersWithoutRoles,
    AllMembers,
}

impl ContentFilter {
    pub const ALL: [ContentFilter; 3] = [
        ContentFilter::Disabled,
        ContentFilter::MembersWithoutRoles,
        ContentFilter::AllMembers,
    ];

    #[must_use]
    pub fn from_code(code: u8) -> ContentFilter {
        ContentFilter::ALL
            .get(usize::from(code))
            .copied()
            .unwrap_or_default()
    }

    #[must_use]
    pub fn code(self) -> u8 {
        ContentFilter::ALL
            .iter()
            .position(|filter| *filter == self)
            .map_or(0, |at| at as u8)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SystemFlags(u32);

impl SystemFlags {
    pub const SUPPRESS_JOIN_NOTIFICATIONS: SystemFlags = SystemFlags(1);
    pub const SUPPRESS_PREMIUM_SUBSCRIPTIONS: SystemFlags = SystemFlags(1 << 1);
    pub const SUPPRESS_GUILD_REMINDER_NOTIFICATIONS: SystemFlags = SystemFlags(1 << 2);
    pub const SUPPRESS_JOIN_NOTIFICATION_REPLIES: SystemFlags = SystemFlags(1 << 3);

    #[must_use]
    pub const fn from_bits(bits: u32) -> SystemFlags {
        SystemFlags(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, other: SystemFlags) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn with(self, other: SystemFlags, on: bool) -> SystemFlags {
        match on {
            true => SystemFlags(self.0 | other.0),
            false => SystemFlags(self.0 & !other.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Incidents {
    pub invites_paused_until: Option<i64>,
    pub directs_paused_until: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceSettings {
    pub id: SpaceId,
    pub name: String,
    pub icon: Option<Picture>,
    pub banner: Option<Picture>,
    pub splash: Option<Picture>,
    pub description: Option<String>,
    pub owner: Option<UserId>,
    pub verification: VerificationLevel,
    pub content_filter: ContentFilter,
    pub mfa_required: bool,
    pub default_notifications: NotifyLevel,
    pub afk_channel: Option<ChannelId>,
    pub afk_timeout: u32,
    pub system_channel: Option<ChannelId>,
    pub system_flags: SystemFlags,
    pub rules_channel: Option<ChannelId>,
    pub updates_channel: Option<ChannelId>,
    pub safety_channel: Option<ChannelId>,
    pub locale: String,
    pub features: Vec<String>,
    pub premium_tier: u8,
    pub boosts: u32,
    pub vanity: Option<String>,
    pub progress_bar: bool,
    pub members: Option<u32>,
    pub incidents: Incidents,
}

impl SpaceSettings {
    #[must_use]
    pub fn has(&self, feature: &str) -> bool {
        self.features.iter().any(|known| known == feature)
    }

    #[must_use]
    pub fn is_community(&self) -> bool {
        self.has(COMMUNITY)
    }

    #[must_use]
    pub fn with_feature(&self, feature: &str, on: bool) -> Vec<String> {
        let mut features: Vec<String> = self
            .features
            .iter()
            .filter(|known| *known != feature)
            .cloned()
            .collect();
        if on {
            features.push(feature.to_owned());
        }
        features
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageUpload {
    pub mime: String,
    pub bytes: Arc<[u8]>,
}

impl ImageUpload {
    #[must_use]
    pub fn new(mime: impl Into<String>, bytes: impl Into<Arc<[u8]>>) -> ImageUpload {
        ImageUpload {
            mime: mime.into(),
            bytes: bytes.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpaceEdit {
    Name(String),
    Icon(Option<ImageUpload>),
    Banner(Option<ImageUpload>),
    Splash(Option<ImageUpload>),
    Description(Option<String>),
    Verification(VerificationLevel),
    ContentFilter(ContentFilter),
    DefaultNotifications(NotifyLevel),
    AfkChannel(Option<ChannelId>),
    AfkTimeout(u32),
    SystemChannel(Option<ChannelId>),
    SystemFlags(SystemFlags),
    RulesChannel(Option<ChannelId>),
    UpdatesChannel(Option<ChannelId>),
    SafetyChannel(Option<ChannelId>),
    Locale(String),
    ProgressBar(bool),
    Features(Vec<String>),
}

pub const AFK_TIMEOUTS: [u32; 5] = [60, 300, 900, 1800, 3600];

pub const LOCALES: [&str; 32] = [
    "id", "da", "de", "en-GB", "en-US", "es-ES", "es-419", "fr", "hr", "it", "lt", "hu", "nl",
    "no", "pl", "pt-BR", "ro", "fi", "sv-SE", "vi", "tr", "cs", "el", "bg", "ru", "uk", "hi", "th",
    "zh-CN", "ja", "zh-TW", "ko",
];
