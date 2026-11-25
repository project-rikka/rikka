use crate::id::{ChannelId, RoleId, RuleId, UserId};
use crate::model::rich::Emoji;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RuleTrigger {
    Keyword,
    Spam,
    KeywordPreset,
    MentionSpam,
    MemberProfile,
}

impl RuleTrigger {
    #[must_use]
    pub fn from_code(code: u8) -> Option<RuleTrigger> {
        match code {
            1 => Some(RuleTrigger::Keyword),
            3 => Some(RuleTrigger::Spam),
            4 => Some(RuleTrigger::KeywordPreset),
            5 => Some(RuleTrigger::MentionSpam),
            6 => Some(RuleTrigger::MemberProfile),
            _ => None,
        }
    }

    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            RuleTrigger::Keyword => 1,
            RuleTrigger::Spam => 3,
            RuleTrigger::KeywordPreset => 4,
            RuleTrigger::MentionSpam => 5,
            RuleTrigger::MemberProfile => 6,
        }
    }

    #[must_use]
    pub const fn event(self) -> u8 {
        match self {
            RuleTrigger::MemberProfile => 2,
            _ => 1,
        }
    }

    #[must_use]
    pub const fn is_singleton(self) -> bool {
        !matches!(self, RuleTrigger::Keyword)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeywordPreset {
    Profanity,
    SexualContent,
    Slurs,
}

impl KeywordPreset {
    pub const ALL: [KeywordPreset; 3] = [
        KeywordPreset::Profanity,
        KeywordPreset::SexualContent,
        KeywordPreset::Slurs,
    ];

    #[must_use]
    pub fn from_code(code: u8) -> Option<KeywordPreset> {
        KeywordPreset::ALL
            .get(usize::from(code).checked_sub(1)?)
            .copied()
    }

    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            KeywordPreset::Profanity => 1,
            KeywordPreset::SexualContent => 2,
            KeywordPreset::Slurs => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleAction {
    Block { message: Option<String> },
    Alert { channel: ChannelId },
    Timeout { seconds: u32 },
    BlockInteractions,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuleTriggerData {
    pub keywords: Vec<String>,
    pub patterns: Vec<String>,
    pub presets: Vec<KeywordPreset>,
    pub allowed: Vec<String>,
    pub mention_limit: Option<u32>,
    pub raid_protection: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AutomodRule {
    pub id: Option<RuleId>,
    pub name: String,
    pub trigger: RuleTrigger,
    pub data: RuleTriggerData,
    pub actions: Vec<RuleAction>,
    pub enabled: bool,
    pub exempt_roles: Vec<RoleId>,
    pub exempt_channels: Vec<ChannelId>,
}

impl AutomodRule {
    #[must_use]
    pub fn draft(trigger: RuleTrigger, name: String) -> AutomodRule {
        let data = match trigger {
            RuleTrigger::MentionSpam => RuleTriggerData {
                mention_limit: Some(MENTION_LIMIT),
                raid_protection: true,
                ..RuleTriggerData::default()
            },
            _ => RuleTriggerData::default(),
        };
        AutomodRule {
            id: None,
            name,
            trigger,
            data,
            actions: vec![RuleAction::Block { message: None }],
            enabled: true,
            exempt_roles: Vec::new(),
            exempt_channels: Vec::new(),
        }
    }
}

pub const MENTION_LIMIT: u32 = 20;
pub const MAX_KEYWORD_RULES: usize = 6;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WelcomeChannel {
    pub channel: ChannelId,
    pub description: String,
    pub emoji: Option<Emoji>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WelcomeScreen {
    pub enabled: bool,
    pub description: String,
    pub channels: Vec<WelcomeChannel>,
}

pub const MAX_WELCOME_CHANNELS: usize = 5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptOption {
    pub id: String,
    pub title: String,
    pub description: String,
    pub emoji: Option<Emoji>,
    pub channels: Vec<ChannelId>,
    pub roles: Vec<RoleId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prompt {
    pub id: String,
    pub title: String,
    pub options: Vec<PromptOption>,
    pub single_select: bool,
    pub required: bool,
    pub in_onboarding: bool,
    pub dropdown: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OnboardingMode {
    #[default]
    Default,
    Advanced,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Onboarding {
    pub enabled: bool,
    pub mode: OnboardingMode,
    pub default_channels: Vec<ChannelId>,
    pub prompts: Vec<Prompt>,
}

pub const MIN_DEFAULT_CHANNELS: usize = 7;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Widget {
    pub enabled: bool,
    pub channel: Option<ChannelId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub uses: u32,
    pub creator: Option<UserId>,
    pub updated: Option<i64>,
    pub dirty: bool,
}

impl Template {
    #[must_use]
    pub fn url(&self) -> String {
        format!("https://discord.new/{}", self.code)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Vanity {
    pub code: Option<String>,
    pub uses: u32,
}
