use crate::id::UserId;
use crate::model::picture::Picture;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub id: UserId,
    pub handle: String,
    pub display: Option<String>,
    pub avatar: Option<Picture>,
    pub bot: bool,
}

impl User {
    #[must_use]
    pub fn name(&self) -> &str {
        self.display
            .as_deref()
            .filter(|display| !display.is_empty())
            .unwrap_or(&self.handle)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Status {
    Online,
    Idle,
    Busy,
    Invisible,
    #[default]
    Offline,
}

impl Status {
    #[must_use]
    pub fn is_present(self) -> bool {
        matches!(self, Status::Online | Status::Idle | Status::Busy)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Presence {
    pub status: Status,
    pub mood: Option<String>,
    pub activity: Option<Activity>,
    pub mobile: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    pub kind: ActivityKind,
    pub name: String,
    pub details: Option<String>,
    pub state: Option<String>,
    pub art: Option<Picture>,
    pub started: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ActivityKind {
    Playing,
    Streaming,
    Listening,
    Watching,
    Competing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Friend {
    pub user: UserId,
    pub bond: Bond,
    pub nickname: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bond {
    Friend,
    Incoming,
    Outgoing,
    Blocked,
}
