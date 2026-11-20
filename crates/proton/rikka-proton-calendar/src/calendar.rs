use jiff::Timestamp;
use rikka_proton_account::AddressId;
use rikka_proton_api::wire_enum;
use serde::{Deserialize, Serialize};

use crate::identifier::{CalendarId, MemberId};
use crate::subscription::Subscription;

wire_enum! {
    #[non_exhaustive]
    pub enum CalendarType: default Personal {
        Personal = 0,
        Subscription = 1,
        Holidays = 2,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Permissions(u32);

impl Permissions {
    pub const SUPER_OWNER: Permissions = Permissions(1);
    pub const OWNER: Permissions = Permissions(2);
    pub const ADMIN: Permissions = Permissions(4);
    pub const READ_MEMBER_LIST: Permissions = Permissions(8);
    pub const WRITE: Permissions = Permissions(16);
    pub const READ: Permissions = Permissions(32);
    pub const AVAILABILITY: Permissions = Permissions(64);
    pub const OWNS: Permissions = Permissions(127);
    pub const EDIT: Permissions = Permissions(112);
    pub const FULL_VIEW: Permissions = Permissions(96);
    pub const LIMITED: Permissions = Permissions(64);

    #[must_use]
    pub fn from_bits(bits: u32) -> Permissions {
        Permissions(bits)
    }

    #[must_use]
    pub fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn contains(self, other: Permissions) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub fn can_write(self) -> bool {
        self.contains(Permissions::WRITE)
    }

    #[must_use]
    pub fn can_read(self) -> bool {
        self.contains(Permissions::READ)
    }

    #[must_use]
    pub fn is_owner(self) -> bool {
        self.contains(Permissions::OWNER)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MemberFlags(u32);

impl MemberFlags {
    pub const ACTIVE: MemberFlags = MemberFlags(1);
    pub const UPDATE_PASSPHRASE: MemberFlags = MemberFlags(2);
    pub const RESET_NEEDED: MemberFlags = MemberFlags(4);
    pub const INCOMPLETE_SETUP: MemberFlags = MemberFlags(8);
    pub const LOST_ACCESS: MemberFlags = MemberFlags(16);
    pub const SELF_DISABLED: MemberFlags = MemberFlags(32);
    pub const SUPER_OWNER_DISABLED: MemberFlags = MemberFlags(64);

    #[must_use]
    pub fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn contains(self, other: MemberFlags) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub fn is_active(self) -> bool {
        self.contains(MemberFlags::ACTIVE)
    }

    #[must_use]
    pub fn needs_setup(self) -> bool {
        self.contains(MemberFlags::INCOMPLETE_SETUP) || self.contains(MemberFlags::RESET_NEEDED)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    #[serde(rename = "ID", default)]
    id: MemberId,
    #[serde(rename = "CalendarID", default)]
    calendar_id: CalendarId,
    #[serde(rename = "AddressID", default)]
    address_id: AddressId,
    #[serde(rename = "Flags", default)]
    flags: MemberFlags,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Description", default)]
    description: String,
    #[serde(rename = "Email", default)]
    email: String,
    #[serde(rename = "Permissions", default)]
    permissions: Permissions,
    #[serde(rename = "Color", default)]
    color: String,
    #[serde(rename = "Display", default, with = "rikka_proton_api::flag")]
    display: bool,
    #[serde(rename = "Priority", default)]
    priority: i32,
}

impl Member {
    #[must_use]
    pub fn id(&self) -> &MemberId {
        &self.id
    }

    #[must_use]
    pub fn calendar_id(&self) -> &CalendarId {
        &self.calendar_id
    }

    #[must_use]
    pub fn address_id(&self) -> &AddressId {
        &self.address_id
    }

    #[must_use]
    pub fn flags(&self) -> MemberFlags {
        self.flags
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
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
    pub fn color(&self) -> &str {
        &self.color
    }

    #[must_use]
    pub fn is_displayed(&self) -> bool {
        self.display
    }

    #[must_use]
    pub fn priority(&self) -> i32 {
        self.priority
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owner {
    #[serde(rename = "Email", default)]
    email: String,
}

impl Owner {
    #[must_use]
    pub fn email(&self) -> &str {
        &self.email
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calendar {
    #[serde(rename = "ID", default)]
    id: CalendarId,
    #[serde(rename = "Type", default)]
    kind: CalendarType,
    #[serde(
        rename = "CreateTime",
        default,
        with = "rikka_proton_api::unix_seconds"
    )]
    create_time: Timestamp,
    #[serde(rename = "Owner", default)]
    owner: Owner,
    #[serde(rename = "Members", default)]
    members: Vec<Member>,
    #[serde(
        rename = "SubscriptionParameters",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    subscription: Option<Subscription>,
}

impl Calendar {
    #[must_use]
    pub fn id(&self) -> &CalendarId {
        &self.id
    }

    #[must_use]
    pub fn kind(&self) -> CalendarType {
        self.kind
    }

    #[must_use]
    pub fn create_time(&self) -> Timestamp {
        self.create_time
    }

    #[must_use]
    pub fn owner(&self) -> &Owner {
        &self.owner
    }

    #[must_use]
    pub fn members(&self) -> &[Member] {
        &self.members
    }

    #[must_use]
    pub fn subscription(&self) -> Option<&Subscription> {
        self.subscription.as_ref()
    }

    pub(crate) fn replace_member(&mut self, member: Member) {
        match self
            .members
            .iter_mut()
            .find(|current| current.id() == member.id())
        {
            Some(current) => *current = member,
            None => self.members.push(member),
        }
    }

    pub(crate) fn remove_member(&mut self, id: &MemberId) {
        self.members.retain(|member| member.id() != id);
    }

    #[must_use]
    pub fn own_member(&self) -> Option<&Member> {
        self.members.first()
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.own_member().map_or("", Member::name)
    }

    #[must_use]
    pub fn color(&self) -> &str {
        self.own_member().map_or("", Member::color)
    }

    #[must_use]
    pub fn permissions(&self) -> Permissions {
        self.own_member()
            .map(Member::permissions)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn is_owned(&self) -> bool {
        self.own_member()
            .is_some_and(|member| member.email().eq_ignore_ascii_case(self.owner.email()))
    }

    #[must_use]
    pub fn is_shared(&self) -> bool {
        self.kind == CalendarType::Personal && !self.is_owned()
    }

    #[must_use]
    pub fn is_writable(&self) -> bool {
        self.kind == CalendarType::Personal && self.permissions().can_write()
    }
}
