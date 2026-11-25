use crate::id::{RoleId, SpaceId, UserId};
use crate::model::notify::SpaceNotify;
use crate::model::permissions::Permissions;
use crate::model::picture::Picture;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Space {
    pub id: SpaceId,
    pub name: String,
    pub icon: Option<Picture>,
    pub banner: Option<Picture>,
    pub owner: Option<UserId>,
    pub muted: bool,
    pub unavailable: bool,
    pub permissions: Permissions,
    pub notify: SpaceNotify,
    pub features: Vec<String>,
    pub premium_tier: u8,
}

impl Space {
    #[must_use]
    pub fn owned_by(&self, user: &UserId) -> bool {
        self.owner.as_ref() == Some(user)
    }

    #[must_use]
    pub fn can(&self, permissions: Permissions) -> bool {
        self.permissions.contains(permissions)
    }

    #[must_use]
    pub fn has(&self, feature: &str) -> bool {
        self.features.iter().any(|known| known == feature)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder {
    pub id: Option<String>,
    pub name: Option<String>,
    pub color: Option<u32>,
    pub spaces: Vec<SpaceId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Role {
    pub id: RoleId,
    pub name: String,
    pub color: Option<u32>,
    pub position: i32,
    pub hoisted: bool,
    pub icon: Option<Picture>,
    pub emoji: Option<String>,
    pub permissions: Permissions,
    pub mentionable: bool,
    pub managed: bool,
}

impl Role {
    #[must_use]
    pub fn is_everyone(&self, space: &SpaceId) -> bool {
        self.id.get() == space.get()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub user: UserId,
    pub space: SpaceId,
    pub nickname: Option<String>,
    pub avatar: Option<Picture>,
    pub roles: Vec<RoleId>,
    pub joined: Option<i64>,
    pub timeout_until: Option<i64>,
}
