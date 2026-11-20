use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::calendar::{Member, Permissions};
use crate::identifier::{
    CalendarId, CalendarKeyId, InvitationId, MemberId, PassphraseId, SettingsId,
};
use crate::notification::Notification;

const KEY_FLAG_ACTIVE: i32 = 1;
const KEY_FLAG_PRIMARY: i32 = 2;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarKey {
    #[serde(rename = "ID", default)]
    id: CalendarKeyId,
    #[serde(rename = "CalendarID", default)]
    calendar_id: CalendarId,
    #[serde(rename = "PrivateKey", default)]
    private_key: String,
    #[serde(rename = "PassphraseID", default)]
    passphrase_id: PassphraseId,
    #[serde(rename = "Flags", default)]
    flags: i32,
}

impl CalendarKey {
    #[must_use]
    pub fn id(&self) -> &CalendarKeyId {
        &self.id
    }

    #[must_use]
    pub fn calendar_id(&self) -> &CalendarId {
        &self.calendar_id
    }

    #[must_use]
    pub fn armored(&self) -> &str {
        &self.private_key
    }

    #[must_use]
    pub fn passphrase_id(&self) -> &PassphraseId {
        &self.passphrase_id
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.flags & KEY_FLAG_ACTIVE != 0
    }

    #[must_use]
    pub fn is_primary(&self) -> bool {
        self.flags & KEY_FLAG_PRIMARY != 0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberPassphrase {
    #[serde(rename = "MemberID", default)]
    member_id: MemberId,
    #[serde(rename = "Passphrase", default)]
    passphrase: String,
    #[serde(rename = "Signature", default)]
    signature: String,
}

impl MemberPassphrase {
    #[must_use]
    pub fn member_id(&self) -> &MemberId {
        &self.member_id
    }

    #[must_use]
    pub fn armored(&self) -> &str {
        &self.passphrase
    }

    #[must_use]
    pub fn signature(&self) -> &str {
        &self.signature
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassphraseInvitation {
    #[serde(rename = "CalendarID", default)]
    calendar_id: CalendarId,
    #[serde(rename = "PassphraseID", default)]
    passphrase_id: PassphraseId,
    #[serde(rename = "InvitationID", default)]
    invitation_id: InvitationId,
    #[serde(rename = "Status", default)]
    status: i32,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(
        rename = "ExpirationTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    expiration_time: Timestamp,
    #[serde(rename = "Permissions", default)]
    permissions: Permissions,
    #[serde(rename = "Email", default)]
    email: String,
}

impl PassphraseInvitation {
    #[must_use]
    pub fn invitation_id(&self) -> &InvitationId {
        &self.invitation_id
    }

    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }

    #[must_use]
    pub fn permissions(&self) -> Permissions {
        self.permissions
    }

    #[must_use]
    pub fn status(&self) -> i32 {
        self.status
    }

    #[must_use]
    pub fn expiration_time(&self) -> Timestamp {
        self.expiration_time
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Passphrase {
    #[serde(rename = "ID", default)]
    id: PassphraseId,
    #[serde(rename = "Flags", default)]
    flags: i32,
    #[serde(rename = "MemberPassphrases", default)]
    member_passphrases: Vec<MemberPassphrase>,
    #[serde(rename = "Invitations", default)]
    invitations: Vec<PassphraseInvitation>,
}

impl Passphrase {
    #[must_use]
    pub fn id(&self) -> &PassphraseId {
        &self.id
    }

    #[must_use]
    pub fn flags(&self) -> i32 {
        self.flags
    }

    #[must_use]
    pub fn member_passphrases(&self) -> &[MemberPassphrase] {
        &self.member_passphrases
    }

    #[must_use]
    pub fn for_member(&self, member: &MemberId) -> Option<&MemberPassphrase> {
        self.member_passphrases
            .iter()
            .find(|candidate| candidate.member_id() == member)
    }

    #[must_use]
    pub fn invitations(&self) -> &[PassphraseInvitation] {
        &self.invitations
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarSettings {
    #[serde(rename = "ID", default)]
    id: SettingsId,
    #[serde(rename = "CalendarID", default)]
    calendar_id: CalendarId,
    #[serde(rename = "DefaultEventDuration", default)]
    default_event_duration: u32,
    #[serde(rename = "DefaultPartDayNotifications", default)]
    part_day_notifications: Vec<Notification>,
    #[serde(rename = "DefaultFullDayNotifications", default)]
    full_day_notifications: Vec<Notification>,
    #[serde(rename = "MakesUserBusy", default, with = "rikka_proton_api::flag")]
    makes_user_busy: bool,
}

impl CalendarSettings {
    #[must_use]
    pub fn id(&self) -> &SettingsId {
        &self.id
    }

    #[must_use]
    pub fn calendar_id(&self) -> &CalendarId {
        &self.calendar_id
    }

    #[must_use]
    pub fn default_event_duration(&self) -> u32 {
        self.default_event_duration
    }

    #[must_use]
    pub fn part_day_notifications(&self) -> &[Notification] {
        &self.part_day_notifications
    }

    #[must_use]
    pub fn full_day_notifications(&self) -> &[Notification] {
        &self.full_day_notifications
    }

    #[must_use]
    pub fn default_notifications(&self, all_day: bool) -> &[Notification] {
        match all_day {
            true => &self.full_day_notifications,
            false => &self.part_day_notifications,
        }
    }

    #[must_use]
    pub fn makes_user_busy(&self) -> bool {
        self.makes_user_busy
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SettingsChange {
    #[serde(
        rename = "DefaultEventDuration",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_event_duration: Option<u32>,
    #[serde(
        rename = "DefaultPartDayNotifications",
        skip_serializing_if = "Option::is_none"
    )]
    pub part_day_notifications: Option<Vec<Notification>>,
    #[serde(
        rename = "DefaultFullDayNotifications",
        skip_serializing_if = "Option::is_none"
    )]
    pub full_day_notifications: Option<Vec<Notification>>,
    #[serde(
        rename = "MakesUserBusy",
        skip_serializing_if = "Option::is_none",
        with = "rikka_proton_api::flag::optional"
    )]
    pub makes_user_busy: Option<bool>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSettings {
    #[serde(rename = "DefaultCalendarID", default)]
    pub default_calendar_id: Option<CalendarId>,
    #[serde(rename = "WeekLength", default)]
    pub week_length: u32,
    #[serde(rename = "DisplayWeekNumber", default, with = "rikka_proton_api::flag")]
    pub display_week_number: bool,
    #[serde(
        rename = "AutoDetectPrimaryTimezone",
        default,
        with = "rikka_proton_api::flag"
    )]
    pub auto_detect_primary_timezone: bool,
    #[serde(rename = "PrimaryTimezone", default)]
    pub primary_timezone: String,
    #[serde(
        rename = "DisplaySecondaryTimezone",
        default,
        with = "rikka_proton_api::flag"
    )]
    pub display_secondary_timezone: bool,
    #[serde(rename = "SecondaryTimezone", default)]
    pub secondary_timezone: Option<String>,
    #[serde(rename = "ViewPreference", default)]
    pub view_preference: i32,
    #[serde(rename = "InviteLocale", default)]
    pub invite_locale: Option<String>,
    #[serde(rename = "AutoImportInvite", default, with = "rikka_proton_api::flag")]
    pub auto_import_invite: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Bootstrap {
    #[serde(rename = "Keys", default)]
    keys: Vec<CalendarKey>,
    #[serde(rename = "Passphrase", default)]
    passphrase: Passphrase,
    #[serde(rename = "Members", default)]
    members: Vec<Member>,
    #[serde(rename = "CalendarSettings", default)]
    settings: CalendarSettings,
}

impl Bootstrap {
    #[must_use]
    pub fn keys(&self) -> &[CalendarKey] {
        &self.keys
    }

    #[must_use]
    pub fn passphrase(&self) -> &Passphrase {
        &self.passphrase
    }

    #[must_use]
    pub fn members(&self) -> &[Member] {
        &self.members
    }

    #[must_use]
    pub fn settings(&self) -> &CalendarSettings {
        &self.settings
    }
}
