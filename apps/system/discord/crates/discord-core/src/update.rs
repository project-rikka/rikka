use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use futures_channel::mpsc;

use crate::id::{ChannelId, MessageId, SpaceId, UserId};
use crate::model::channel::Channel;
use crate::model::message::{Embed, Message, Reaction, Sticker};
use crate::model::rich::Emoji;
use crate::model::space::{Folder, Member, Role, Space};
use crate::model::user::{Friend, Presence, User};
use crate::model::voice::VoiceState;

#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    Link(LinkState),
    Ready(Box<Ready>),
    Me(User),
    Space(Space),
    SpaceGone(SpaceId),
    Folders(Vec<Folder>),
    Roles {
        space: SpaceId,
        roles: Vec<Role>,
    },
    Emojis {
        space: SpaceId,
        emojis: Vec<Emoji>,
    },
    Stickers {
        space: SpaceId,
        stickers: Vec<Sticker>,
    },
    Channel(Channel),
    ChannelGone(ChannelId),
    Users(Vec<User>),
    Members {
        space: SpaceId,
        members: Vec<Member>,
    },
    MemberGone {
        space: SpaceId,
        user: UserId,
    },
    Presence {
        user: UserId,
        presence: Presence,
    },
    Friend(Friend),
    FriendGone(UserId),
    Message(Message),
    Embeds {
        channel: ChannelId,
        message: MessageId,
        embeds: Vec<Embed>,
    },
    Deleted {
        channel: ChannelId,
        messages: Vec<MessageId>,
    },
    Reaction(ReactionChange),
    Typing {
        channel: ChannelId,
        user: UserId,
        at: Instant,
    },
    Unread(Unread),
    Voice(VoiceState),
    VoiceGone {
        user: UserId,
        space: Option<SpaceId>,
    },
    Call(Ringing),
    CallGone(ChannelId),
    Roster {
        channel: ChannelId,
        roster: Arc<Roster>,
    },
    Notify(Notice),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkState {
    Connecting,
    Online,
    Reconnecting { attempt: u32 },
    SignedOut,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ready {
    pub me: User,
    pub spaces: Vec<Space>,
    pub folders: Vec<Folder>,
    pub roles: Vec<(SpaceId, Vec<Role>)>,
    pub emojis: Vec<(SpaceId, Vec<Emoji>)>,
    pub stickers: Vec<(SpaceId, Vec<Sticker>)>,
    pub channels: Vec<Channel>,
    pub users: Vec<User>,
    pub members: Vec<Member>,
    pub presences: Vec<(UserId, Presence)>,
    pub friends: Vec<Friend>,
    pub unread: Vec<Unread>,
    pub voice: Vec<VoiceState>,
    pub calls: Vec<Ringing>,
    pub own_presence: Presence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ringing {
    pub channel: ChannelId,
    pub ringing: Vec<UserId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unread {
    pub channel: ChannelId,
    pub unread: bool,
    pub mentions: u32,
    pub last_read: Option<MessageId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReactionChange {
    pub channel: ChannelId,
    pub message: MessageId,
    pub kind: ReactionKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReactionKind {
    Added { emoji: Emoji, me: bool },
    Removed { emoji: Emoji, me: bool },
    Cleared(Option<Emoji>),
}

impl ReactionKind {
    pub fn apply(self, reactions: &mut Vec<Reaction>) {
        match self {
            ReactionKind::Added { emoji, me } => {
                match reactions
                    .iter_mut()
                    .find(|reaction| reaction.emoji.same(&emoji))
                {
                    Some(reaction) => {
                        if !(me && reaction.me) {
                            reaction.count += 1;
                        }
                        reaction.me |= me;
                    }
                    None => reactions.push(Reaction {
                        emoji,
                        count: 1,
                        me,
                    }),
                }
            }
            ReactionKind::Removed { emoji, me } => {
                if let Some(at) = reactions
                    .iter()
                    .position(|reaction| reaction.emoji.same(&emoji))
                {
                    let reaction = &mut reactions[at];
                    if !me || reaction.me {
                        reaction.count = reaction.count.saturating_sub(1);
                    }
                    if me {
                        reaction.me = false;
                    }
                    if reaction.count == 0 {
                        reactions.remove(at);
                    }
                }
            }
            ReactionKind::Cleared(None) => reactions.clear(),
            ReactionKind::Cleared(Some(emoji)) => {
                reactions.retain(|reaction| !reaction.emoji.same(&emoji));
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Roster {
    pub len: u32,
    pub known: BTreeMap<u32, RosterRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RosterRow {
    Group { label: RosterGroup, count: u32 },
    Member(UserId),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RosterGroup {
    Role(crate::id::RoleId),
    Online,
    Offline,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub space: Option<SpaceId>,
    pub message: Arc<Message>,
}

pub type Updates = mpsc::UnboundedReceiver<Update>;

#[derive(Clone, Debug)]
pub struct Feed(mpsc::UnboundedSender<Update>);

impl Feed {
    #[must_use]
    pub fn new() -> (Feed, Updates) {
        let (sender, receiver) = mpsc::unbounded();
        (Feed(sender), receiver)
    }

    pub fn send(&self, update: Update) {
        let _ = self.0.unbounded_send(update);
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.0.is_closed()
    }
}
