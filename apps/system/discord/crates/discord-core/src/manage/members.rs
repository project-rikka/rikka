use crate::id::{AuditEntryId, ChannelId, RoleId, UserId};
use crate::manage::space::ImageUpload;
use crate::model::permissions::Permissions;
use crate::model::space::Member;
use crate::model::user::User;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoleEdit {
    Name(String),
    Color(Option<u32>),
    Hoist(bool),
    Mentionable(bool),
    Permissions(Permissions),
    Icon(Option<ImageUpload>),
    Emoji(Option<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemberEdit {
    Nickname(Option<String>),
    Roles(Vec<RoleId>),
    Timeout(Option<i64>),
    Mute(bool),
    Deaf(bool),
    Channel(Option<ChannelId>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberHit {
    pub member: Member,
    pub user: User,
    pub invite: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemberPage {
    pub hits: Vec<MemberHit>,
    pub total: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ban {
    pub user: User,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PurgeSpan {
    None,
    Hour,
    Hours6,
    Hours12,
    Day,
    Days3,
    Days7,
}

impl PurgeSpan {
    pub const ALL: [PurgeSpan; 7] = [
        PurgeSpan::None,
        PurgeSpan::Hour,
        PurgeSpan::Hours6,
        PurgeSpan::Hours12,
        PurgeSpan::Day,
        PurgeSpan::Days3,
        PurgeSpan::Days7,
    ];

    #[must_use]
    pub const fn seconds(self) -> u32 {
        match self {
            PurgeSpan::None => 0,
            PurgeSpan::Hour => 3600,
            PurgeSpan::Hours6 => 6 * 3600,
            PurgeSpan::Hours12 => 12 * 3600,
            PurgeSpan::Day => 24 * 3600,
            PurgeSpan::Days3 => 3 * 24 * 3600,
            PurgeSpan::Days7 => 7 * 24 * 3600,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TimeoutSpan {
    Minute,
    Minutes5,
    Minutes10,
    Hour,
    Day,
    Week,
}

impl TimeoutSpan {
    pub const ALL: [TimeoutSpan; 6] = [
        TimeoutSpan::Minute,
        TimeoutSpan::Minutes5,
        TimeoutSpan::Minutes10,
        TimeoutSpan::Hour,
        TimeoutSpan::Day,
        TimeoutSpan::Week,
    ];

    #[must_use]
    pub const fn seconds(self) -> i64 {
        match self {
            TimeoutSpan::Minute => 60,
            TimeoutSpan::Minutes5 => 300,
            TimeoutSpan::Minutes10 => 600,
            TimeoutSpan::Hour => 3600,
            TimeoutSpan::Day => 86_400,
            TimeoutSpan::Week => 604_800,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prune {
    pub days: u8,
    pub roles: Vec<RoleId>,
}

pub const PRUNE_DAYS: [u8; 2] = [7, 30];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AuditAction(pub u16);

impl AuditAction {
    pub const GUILD_UPDATE: AuditAction = AuditAction(1);
    pub const CHANNEL_CREATE: AuditAction = AuditAction(10);
    pub const CHANNEL_UPDATE: AuditAction = AuditAction(11);
    pub const CHANNEL_DELETE: AuditAction = AuditAction(12);
    pub const CHANNEL_OVERWRITE_CREATE: AuditAction = AuditAction(13);
    pub const CHANNEL_OVERWRITE_UPDATE: AuditAction = AuditAction(14);
    pub const CHANNEL_OVERWRITE_DELETE: AuditAction = AuditAction(15);
    pub const MEMBER_KICK: AuditAction = AuditAction(20);
    pub const MEMBER_PRUNE: AuditAction = AuditAction(21);
    pub const MEMBER_BAN_ADD: AuditAction = AuditAction(22);
    pub const MEMBER_BAN_REMOVE: AuditAction = AuditAction(23);
    pub const MEMBER_UPDATE: AuditAction = AuditAction(24);
    pub const MEMBER_ROLE_UPDATE: AuditAction = AuditAction(25);
    pub const MEMBER_MOVE: AuditAction = AuditAction(26);
    pub const MEMBER_DISCONNECT: AuditAction = AuditAction(27);
    pub const BOT_ADD: AuditAction = AuditAction(28);
    pub const ROLE_CREATE: AuditAction = AuditAction(30);
    pub const ROLE_UPDATE: AuditAction = AuditAction(31);
    pub const ROLE_DELETE: AuditAction = AuditAction(32);
    pub const INVITE_CREATE: AuditAction = AuditAction(40);
    pub const INVITE_UPDATE: AuditAction = AuditAction(41);
    pub const INVITE_DELETE: AuditAction = AuditAction(42);
    pub const WEBHOOK_CREATE: AuditAction = AuditAction(50);
    pub const WEBHOOK_UPDATE: AuditAction = AuditAction(51);
    pub const WEBHOOK_DELETE: AuditAction = AuditAction(52);
    pub const EMOJI_CREATE: AuditAction = AuditAction(60);
    pub const EMOJI_UPDATE: AuditAction = AuditAction(61);
    pub const EMOJI_DELETE: AuditAction = AuditAction(62);
    pub const MESSAGE_DELETE: AuditAction = AuditAction(72);
    pub const MESSAGE_BULK_DELETE: AuditAction = AuditAction(73);
    pub const MESSAGE_PIN: AuditAction = AuditAction(74);
    pub const MESSAGE_UNPIN: AuditAction = AuditAction(75);
    pub const INTEGRATION_CREATE: AuditAction = AuditAction(80);
    pub const INTEGRATION_UPDATE: AuditAction = AuditAction(81);
    pub const INTEGRATION_DELETE: AuditAction = AuditAction(82);
    pub const STAGE_INSTANCE_CREATE: AuditAction = AuditAction(83);
    pub const STAGE_INSTANCE_UPDATE: AuditAction = AuditAction(84);
    pub const STAGE_INSTANCE_DELETE: AuditAction = AuditAction(85);
    pub const STICKER_CREATE: AuditAction = AuditAction(90);
    pub const STICKER_UPDATE: AuditAction = AuditAction(91);
    pub const STICKER_DELETE: AuditAction = AuditAction(92);
    pub const EVENT_CREATE: AuditAction = AuditAction(100);
    pub const EVENT_UPDATE: AuditAction = AuditAction(101);
    pub const EVENT_DELETE: AuditAction = AuditAction(102);
    pub const THREAD_CREATE: AuditAction = AuditAction(110);
    pub const THREAD_UPDATE: AuditAction = AuditAction(111);
    pub const THREAD_DELETE: AuditAction = AuditAction(112);
    pub const COMMAND_PERMISSION_UPDATE: AuditAction = AuditAction(121);
    pub const SOUND_CREATE: AuditAction = AuditAction(130);
    pub const SOUND_UPDATE: AuditAction = AuditAction(131);
    pub const SOUND_DELETE: AuditAction = AuditAction(132);
    pub const AUTOMOD_RULE_CREATE: AuditAction = AuditAction(140);
    pub const AUTOMOD_RULE_UPDATE: AuditAction = AuditAction(141);
    pub const AUTOMOD_RULE_DELETE: AuditAction = AuditAction(142);
    pub const AUTOMOD_BLOCK_MESSAGE: AuditAction = AuditAction(143);
    pub const AUTOMOD_FLAG_TO_CHANNEL: AuditAction = AuditAction(144);
    pub const AUTOMOD_TIMEOUT: AuditAction = AuditAction(145);
    pub const ONBOARDING_PROMPT_CREATE: AuditAction = AuditAction(163);
    pub const ONBOARDING_PROMPT_UPDATE: AuditAction = AuditAction(164);
    pub const ONBOARDING_PROMPT_DELETE: AuditAction = AuditAction(165);
    pub const ONBOARDING_CREATE: AuditAction = AuditAction(166);
    pub const ONBOARDING_UPDATE: AuditAction = AuditAction(167);
    pub const HOME_SETTINGS_CREATE: AuditAction = AuditAction(190);
    pub const HOME_SETTINGS_UPDATE: AuditAction = AuditAction(191);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditChange {
    pub key: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditEntry {
    pub id: AuditEntryId,
    pub actor: Option<User>,
    pub target: Option<String>,
    pub action: AuditAction,
    pub changes: Vec<AuditChange>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuditQuery {
    pub before: Option<AuditEntryId>,
    pub actor: Option<UserId>,
    pub action: Option<AuditAction>,
}

pub const AUDIT_PAGE: u32 = 50;
