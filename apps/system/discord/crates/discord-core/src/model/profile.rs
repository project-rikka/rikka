use crate::id::{RoleId, SpaceId};
use crate::model::picture::Picture;
use crate::model::rich::Rich;
use crate::model::user::User;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub user: User,
    pub bio: Rich,
    pub pronouns: Option<String>,
    pub banner: Option<Picture>,
    pub accent: Option<u32>,
    pub theme: Option<u32>,
    pub badges: Vec<Badge>,
    pub space: Option<SpaceProfile>,
    pub mutual_spaces: Vec<SpaceId>,
    pub mutual_friends: Vec<User>,
    pub connections: Vec<Connected>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badge {
    pub id: String,
    pub description: String,
    pub icon: Picture,
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceProfile {
    pub space: SpaceId,
    pub nickname: Option<String>,
    pub avatar: Option<Picture>,
    pub roles: Vec<RoleId>,
    pub joined: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connected {
    pub kind: String,
    pub name: String,
    pub url: Option<String>,
    pub verified: bool,
}
