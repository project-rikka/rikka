use crate::id::{ChannelId, SpaceId, TagId, UserId};
use crate::model::picture::Picture;
use crate::model::rich::Emoji;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    pub id: ChannelId,
    pub space: Option<SpaceId>,
    pub kind: ChannelKind,
    pub name: String,
    pub topic: Option<String>,
    pub parent: Option<ChannelId>,
    pub position: i32,
    pub icon: Option<Picture>,
    pub recipients: Vec<UserId>,
    pub abilities: Abilities,
    pub nsfw: bool,
    pub muted: bool,
    pub slowmode: u32,
    pub last_activity: Option<i64>,
    pub thread: Option<Thread>,
    pub tags: Vec<ForumTag>,
    pub bitrate: Option<u32>,
    pub user_limit: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChannelKind {
    Text,
    Announcement,
    Voice,
    Stage,
    Category,
    Forum,
    Thread,
    Direct,
    Group,
}

impl ChannelKind {
    #[must_use]
    pub fn is_private(self) -> bool {
        matches!(self, ChannelKind::Direct | ChannelKind::Group)
    }

    #[must_use]
    pub fn holds_messages(self) -> bool {
        matches!(
            self,
            ChannelKind::Text
                | ChannelKind::Announcement
                | ChannelKind::Voice
                | ChannelKind::Thread
                | ChannelKind::Direct
                | ChannelKind::Group
        )
    }

    #[must_use]
    pub fn is_voice(self) -> bool {
        matches!(self, ChannelKind::Voice | ChannelKind::Stage)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thread {
    pub owner: Option<UserId>,
    pub archived: bool,
    pub locked: bool,
    pub joined: bool,
    pub messages: u32,
    pub members: u32,
    pub applied_tags: Vec<TagId>,
    pub created: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumTag {
    pub id: TagId,
    pub name: String,
    pub emoji: Option<Emoji>,
    pub moderated: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Abilities(u32);

impl Abilities {
    pub const VIEW: Abilities = Abilities(1);
    pub const READ_HISTORY: Abilities = Abilities(1 << 1);
    pub const SEND: Abilities = Abilities(1 << 2);
    pub const SEND_IN_THREADS: Abilities = Abilities(1 << 3);
    pub const ATTACH: Abilities = Abilities(1 << 4);
    pub const EMBED_LINKS: Abilities = Abilities(1 << 5);
    pub const REACT: Abilities = Abilities(1 << 6);
    pub const EXTERNAL_EMOJI: Abilities = Abilities(1 << 7);
    pub const MENTION_EVERYONE: Abilities = Abilities(1 << 8);
    pub const MANAGE_MESSAGES: Abilities = Abilities(1 << 9);
    pub const PIN: Abilities = Abilities(1 << 10);
    pub const CREATE_THREADS: Abilities = Abilities(1 << 11);
    pub const CONNECT: Abilities = Abilities(1 << 12);
    pub const SPEAK: Abilities = Abilities(1 << 13);
    pub const VOICE_ACTIVITY: Abilities = Abilities(1 << 14);
    pub const MUTE_MEMBERS: Abilities = Abilities(1 << 15);
    pub const STICKERS: Abilities = Abilities(1 << 16);
    pub const STREAM: Abilities = Abilities(1 << 17);

    pub const ALL: Abilities = Abilities((1 << 18) - 1);
    pub const NONE: Abilities = Abilities(0);

    #[must_use]
    pub const fn contains(self, other: Abilities) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn union(self, other: Abilities) -> Abilities {
        Abilities(self.0 | other.0)
    }

    #[must_use]
    pub const fn without(self, other: Abilities) -> Abilities {
        Abilities(self.0 & !other.0)
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }
}

impl std::ops::BitOr for Abilities {
    type Output = Abilities;

    fn bitor(self, other: Abilities) -> Abilities {
        self.union(other)
    }
}

impl std::ops::BitOrAssign for Abilities {
    fn bitor_assign(&mut self, other: Abilities) {
        *self = self.union(other);
    }
}

impl std::fmt::Debug for Abilities {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Abilities({:#x})", self.0)
    }
}
