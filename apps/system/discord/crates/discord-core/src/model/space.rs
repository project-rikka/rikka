use crate::id::{RoleId, SpaceId, UserId};
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub user: UserId,
    pub space: SpaceId,
    pub nickname: Option<String>,
    pub avatar: Option<Picture>,
    pub roles: Vec<RoleId>,
    pub joined: Option<i64>,
}
