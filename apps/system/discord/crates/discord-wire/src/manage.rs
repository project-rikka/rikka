use serde::Deserialize;
use serde_json::Value;

use crate::flex;
use crate::guild::Member;
use crate::user::User;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Ban {
    pub user: User,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AuditLog {
    #[serde(deserialize_with = "flex::list")]
    pub audit_log_entries: Vec<AuditEntry>,
    #[serde(deserialize_with = "flex::list")]
    pub users: Vec<User>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AuditEntry {
    pub id: String,
    #[serde(deserialize_with = "flex::id")]
    pub user_id: Option<String>,
    #[serde(deserialize_with = "flex::id")]
    pub target_id: Option<String>,
    pub action_type: u16,
    #[serde(deserialize_with = "flex::list")]
    pub changes: Vec<AuditChange>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AuditChange {
    pub key: String,
    pub new_value: Option<Value>,
    pub old_value: Option<Value>,
}

impl AuditChange {
    #[must_use]
    pub fn before(&self) -> Option<String> {
        self.old_value.as_ref().and_then(shown)
    }

    #[must_use]
    pub fn after(&self) -> Option<String> {
        self.new_value.as_ref().and_then(shown)
    }
}

fn shown(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => Some(
            items
                .iter()
                .map(|item| match item.get("name").and_then(Value::as_str) {
                    Some(name) => name.to_owned(),
                    None => item.to_string(),
                })
                .collect::<Vec<_>>()
                .join(", "),
        ),
        other => Some(other.to_string()),
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Webhook {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: u8,
    pub name: Option<String>,
    pub avatar: Option<String>,
    #[serde(deserialize_with = "flex::id")]
    pub channel_id: Option<String>,
    pub token: Option<String>,
    pub user: Option<User>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Integration {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(deserialize_with = "flex::flag")]
    pub enabled: bool,
    pub account: Option<IntegrationAccount>,
    pub application: Option<IntegrationApplication>,
    pub user: Option<User>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct IntegrationAccount {
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct IntegrationApplication {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AutomodRule {
    pub id: String,
    pub name: String,
    pub event_type: u8,
    pub trigger_type: u8,
    pub trigger_metadata: TriggerMetadata,
    #[serde(deserialize_with = "flex::list")]
    pub actions: Vec<AutomodAction>,
    #[serde(deserialize_with = "flex::flag")]
    pub enabled: bool,
    #[serde(deserialize_with = "flex::snowflakes")]
    pub exempt_roles: Vec<String>,
    #[serde(deserialize_with = "flex::snowflakes")]
    pub exempt_channels: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct TriggerMetadata {
    #[serde(deserialize_with = "flex::list")]
    pub keyword_filter: Vec<String>,
    #[serde(deserialize_with = "flex::list")]
    pub regex_patterns: Vec<String>,
    #[serde(deserialize_with = "flex::list")]
    pub presets: Vec<u8>,
    #[serde(deserialize_with = "flex::list")]
    pub allow_list: Vec<String>,
    pub mention_total_limit: Option<u32>,
    #[serde(deserialize_with = "flex::flag")]
    pub mention_raid_protection_enabled: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct AutomodAction {
    #[serde(rename = "type")]
    pub kind: u8,
    pub metadata: ActionMetadata,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ActionMetadata {
    #[serde(deserialize_with = "flex::id")]
    pub channel_id: Option<String>,
    pub duration_seconds: Option<u32>,
    pub custom_message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct WelcomeScreen {
    pub description: Option<String>,
    #[serde(deserialize_with = "flex::list")]
    pub welcome_channels: Vec<WelcomeChannel>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct WelcomeChannel {
    #[serde(deserialize_with = "flex::snowflake")]
    pub channel_id: String,
    pub description: String,
    #[serde(deserialize_with = "flex::id")]
    pub emoji_id: Option<String>,
    pub emoji_name: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Onboarding {
    #[serde(deserialize_with = "flex::list")]
    pub prompts: Vec<Prompt>,
    #[serde(deserialize_with = "flex::snowflakes")]
    pub default_channel_ids: Vec<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub enabled: bool,
    pub mode: u8,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Prompt {
    #[serde(deserialize_with = "flex::snowflake")]
    pub id: String,
    #[serde(rename = "type")]
    pub kind: u8,
    pub title: String,
    #[serde(deserialize_with = "flex::list")]
    pub options: Vec<PromptOption>,
    #[serde(deserialize_with = "flex::flag")]
    pub single_select: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub required: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub in_onboarding: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PromptOption {
    #[serde(deserialize_with = "flex::snowflake")]
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub emoji: Option<crate::guild::Emoji>,
    #[serde(deserialize_with = "flex::snowflakes")]
    pub channel_ids: Vec<String>,
    #[serde(deserialize_with = "flex::snowflakes")]
    pub role_ids: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Widget {
    #[serde(deserialize_with = "flex::flag")]
    pub enabled: bool,
    #[serde(deserialize_with = "flex::id")]
    pub channel_id: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Template {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub usage_count: u32,
    #[serde(deserialize_with = "flex::id")]
    pub creator_id: Option<String>,
    pub updated_at: Option<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub is_dirty: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Vanity {
    pub code: Option<String>,
    pub uses: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MemberSearch {
    #[serde(deserialize_with = "flex::list")]
    pub members: Vec<MemberSearchHit>,
    pub total_result_count: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MemberSearchHit {
    pub member: Member,
    pub source_invite_code: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct PruneCount {
    pub pruned: Option<u32>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MfaRequired {
    pub mfa: Option<MfaTicket>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MfaTicket {
    pub ticket: String,
    #[serde(deserialize_with = "flex::list")]
    pub methods: Vec<MfaOffer>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MfaOffer {
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct MfaToken {
    pub token: String,
}

pub const MFA_REQUIRED: u64 = 60003;

pub mod automod_action {
    pub const BLOCK: u8 = 1;
    pub const ALERT: u8 = 2;
    pub const TIMEOUT: u8 = 3;
    pub const BLOCK_INTERACTIONS: u8 = 4;
}

pub mod overwrite_kind {
    pub const ROLE: u8 = 0;
    pub const MEMBER: u8 = 1;
}

pub mod video_quality {
    pub const AUTO: u8 = 1;
    pub const FULL: u8 = 2;
}

pub mod mfa_level {
    pub const NONE: u8 = 0;
    pub const ELEVATED: u8 = 1;
}

pub mod onboarding_mode {
    pub const DEFAULT: u8 = 0;
    pub const ADVANCED: u8 = 1;
}

pub mod prompt_kind {
    pub const MULTIPLE_CHOICE: u8 = 0;
    pub const DROPDOWN: u8 = 1;
}

pub mod webhook_kind {
    pub const INCOMING: u8 = 1;
}
