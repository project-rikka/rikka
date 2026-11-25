use crate::id::{ChannelId, RoleId, SpaceId, UserId};
use crate::model::channel::ChannelKind;
use crate::model::permissions::Permissions;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OverwriteTarget {
    Role(RoleId),
    Member(UserId),
}

impl OverwriteTarget {
    #[must_use]
    pub fn raw(self) -> u64 {
        match self {
            OverwriteTarget::Role(role) => role.get(),
            OverwriteTarget::Member(user) => user.get(),
        }
    }

    #[must_use]
    pub fn is_everyone(self, space: &SpaceId) -> bool {
        matches!(self, OverwriteTarget::Role(role) if role.get() == space.get())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Overwrite {
    pub target: OverwriteTarget,
    pub allow: Permissions,
    pub deny: Permissions,
}

impl Overwrite {
    #[must_use]
    pub fn empty(target: OverwriteTarget) -> Overwrite {
        Overwrite {
            target,
            allow: Permissions::NONE,
            deny: Permissions::NONE,
        }
    }

    #[must_use]
    pub fn state(&self, permission: Permissions) -> Grant {
        match () {
            () if self.allow.contains(permission) => Grant::Allow,
            () if self.deny.contains(permission) => Grant::Deny,
            () => Grant::Inherit,
        }
    }

    #[must_use]
    pub fn with(self, permission: Permissions, grant: Grant) -> Overwrite {
        let allow = self.allow.without(permission);
        let deny = self.deny.without(permission);
        match grant {
            Grant::Allow => Overwrite {
                allow: allow | permission,
                deny,
                ..self
            },
            Grant::Deny => Overwrite {
                allow,
                deny: deny | permission,
                ..self
            },
            Grant::Inherit => Overwrite {
                allow,
                deny,
                ..self
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Grant {
    Deny,
    Inherit,
    Allow,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum VideoQuality {
    #[default]
    Auto,
    Full,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelSettings {
    pub id: ChannelId,
    pub space: SpaceId,
    pub kind: ChannelKind,
    pub name: String,
    pub topic: String,
    pub nsfw: bool,
    pub slowmode: u32,
    pub bitrate: u32,
    pub user_limit: u32,
    pub region: Option<String>,
    pub video_quality: VideoQuality,
    pub parent: Option<ChannelId>,
    pub overwrites: Vec<Overwrite>,
    pub archive_minutes: u32,
    pub thread_slowmode: u32,
}

impl ChannelSettings {
    #[must_use]
    pub fn overwrite(&self, target: OverwriteTarget) -> Option<&Overwrite> {
        self.overwrites
            .iter()
            .find(|overwrite| overwrite.target == target)
    }

    #[must_use]
    pub fn is_private(&self) -> bool {
        let everyone = OverwriteTarget::Role(RoleId::new(self.space.get()));
        self.overwrite(everyone)
            .is_some_and(|overwrite| overwrite.deny.contains(Permissions::VIEW_CHANNEL))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChannelEdit {
    Name(String),
    Topic(String),
    Nsfw(bool),
    Slowmode(u32),
    Bitrate(u32),
    UserLimit(u32),
    Region(Option<String>),
    VideoQuality(VideoQuality),
    ArchiveMinutes(u32),
    ThreadSlowmode(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChannelDraft {
    pub name: String,
    pub kind: ChannelKind,
    pub parent: Option<ChannelId>,
    pub private: bool,
}

pub const SLOWMODES: [u32; 15] = [
    0, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 10800, 21600,
];
pub const ARCHIVE_MINUTES: [u32; 4] = [60, 1440, 4320, 10080];
pub const MIN_BITRATE: u32 = 8000;
pub const MAX_USER_LIMIT: u32 = 99;

#[must_use]
pub const fn max_bitrate(tier: u8) -> u32 {
    match tier {
        1 => 128_000,
        2 => 256_000,
        3 => 384_000,
        _ => 96_000,
    }
}
