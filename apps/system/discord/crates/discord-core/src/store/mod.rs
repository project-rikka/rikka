mod layout;
mod timeline;

use std::cmp::Reverse;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::id::{ChannelId, MessageId, RoleId, SpaceId, UserId};
use crate::model::channel::{Channel, ChannelKind};
use crate::model::message::{Delivery, Message, MessageKind, Quoted, Reply, Sticker};
use crate::model::rich::{Emoji, Rich};
use crate::model::space::{Folder, Member, Role, Space};
use crate::model::user::{Bond, Friend, Presence, User};
use crate::model::voice::VoiceState;
use crate::requests::{Draft, Page};
use crate::update::{LinkState, Ready, Ringing, Roster, Unread, Update};

pub use layout::{RailItem, Row, Section};
pub use timeline::Timeline;

const TYPING_LASTS: Duration = Duration::from_secs(10);
const KEPT_TIMELINES: usize = 24;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Area {
    Everything,
    Link,
    Me,
    Rail,
    Space(SpaceId),
    Channels(Option<SpaceId>),
    Channel(ChannelId),
    Timeline(ChannelId),
    User(UserId),
    Members(SpaceId),
    Member(SpaceId, UserId),
    Presence(UserId),
    Friends,
    Unread(ChannelId),
    Unseen(Option<SpaceId>),
    Voice(ChannelId),
    Calls,
    Roster(ChannelId),
    Typing(ChannelId),
}

impl Area {
    #[must_use]
    pub fn channel(&self) -> Option<&ChannelId> {
        match self {
            Area::Channel(channel)
            | Area::Timeline(channel)
            | Area::Unread(channel)
            | Area::Voice(channel)
            | Area::Roster(channel)
            | Area::Typing(channel) => Some(channel),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Unseen {
    pub unread: bool,
    pub mentions: u32,
}

#[derive(Clone, Debug)]
pub struct Store {
    link: LinkState,
    me: Option<User>,
    own_presence: Presence,
    spaces: HashMap<SpaceId, Space>,
    folders: Vec<Folder>,
    roles: HashMap<SpaceId, Vec<Role>>,
    emojis: HashMap<SpaceId, Vec<Emoji>>,
    stickers: HashMap<SpaceId, Vec<Sticker>>,
    channels: HashMap<ChannelId, Channel>,
    places: HashMap<Option<SpaceId>, HashSet<ChannelId>>,
    users: HashMap<UserId, User>,
    members: HashMap<SpaceId, HashMap<UserId, Member>>,
    presences: HashMap<UserId, Presence>,
    friends: HashMap<UserId, Friend>,
    unread: HashMap<ChannelId, Unread>,
    voice: HashMap<(Option<SpaceId>, UserId), VoiceState>,
    calls: HashMap<ChannelId, Ringing>,
    rosters: HashMap<ChannelId, Arc<Roster>>,
    timelines: HashMap<ChannelId, Timeline>,
    visited: VecDeque<ChannelId>,
    typing: HashMap<ChannelId, Vec<(UserId, Instant)>>,
}

impl Default for Store {
    fn default() -> Store {
        Store {
            link: LinkState::Connecting,
            me: None,
            own_presence: Presence::default(),
            spaces: HashMap::new(),
            folders: Vec::new(),
            roles: HashMap::new(),
            emojis: HashMap::new(),
            stickers: HashMap::new(),
            channels: HashMap::new(),
            places: HashMap::new(),
            users: HashMap::new(),
            members: HashMap::new(),
            presences: HashMap::new(),
            friends: HashMap::new(),
            unread: HashMap::new(),
            voice: HashMap::new(),
            calls: HashMap::new(),
            rosters: HashMap::new(),
            timelines: HashMap::new(),
            visited: VecDeque::new(),
            typing: HashMap::new(),
        }
    }
}

impl Store {
    pub fn apply(&mut self, update: Update) -> Vec<Area> {
        match update {
            Update::Link(link) => {
                if self.link == link {
                    return Vec::new();
                }
                self.link = link;
                vec![Area::Link]
            }
            Update::Ready(ready) => {
                self.ready(*ready);
                vec![Area::Everything]
            }
            Update::Me(me) => {
                if self.me.as_ref() == Some(&me) {
                    return Vec::new();
                }
                let mut areas = vec![Area::Me];
                areas.extend(self.put_user(me.clone()));
                self.me = Some(me);
                areas
            }
            Update::Space(space) => {
                if self.spaces.get(&space.id) == Some(&space) {
                    return Vec::new();
                }
                let id = space.id;
                self.spaces.insert(id, space);
                vec![Area::Rail, Area::Space(id), Area::Unseen(Some(id))]
            }
            Update::SpaceGone(space) => self.forget_space(&space),
            Update::Folders(folders) => {
                if self.folders == folders {
                    return Vec::new();
                }
                self.folders = folders;
                vec![Area::Rail]
            }
            Update::Roles { space, mut roles } => {
                sort_roles(&mut roles);
                replaced(&mut self.roles, space, roles, Area::Space)
            }
            Update::Emojis { space, emojis } => {
                replaced(&mut self.emojis, space, emojis, Area::Space)
            }
            Update::Stickers { space, stickers } => {
                replaced(&mut self.stickers, space, stickers, Area::Space)
            }
            Update::Channel(channel) => self.put_channel(channel),
            Update::ChannelGone(channel) => self.forget_channel(&channel),
            Update::Users(users) => users
                .into_iter()
                .flat_map(|user| self.put_user(user))
                .collect(),
            Update::Members { space, members } => {
                let known = self.members.entry(space).or_default();
                let mut areas: Vec<Area> = members
                    .into_iter()
                    .filter_map(|member| {
                        if known.get(&member.user) == Some(&member) {
                            return None;
                        }
                        let user = member.user;
                        known.insert(user, member);
                        Some(Area::Member(space, user))
                    })
                    .collect();
                if !areas.is_empty() {
                    areas.push(Area::Members(space));
                }
                areas
            }
            Update::MemberGone { space, user } => {
                match self
                    .members
                    .get_mut(&space)
                    .and_then(|known| known.remove(&user))
                {
                    Some(_) => vec![Area::Member(space, user), Area::Members(space)],
                    None => Vec::new(),
                }
            }
            Update::Presence { user, presence } => {
                if self.is_me(&user) {
                    self.own_presence = presence.clone();
                }
                if self.presences.get(&user) == Some(&presence) {
                    return Vec::new();
                }
                self.presences.insert(user, presence);
                vec![Area::Presence(user)]
            }
            Update::Friend(friend) => {
                if self.friends.get(&friend.user) == Some(&friend) {
                    return Vec::new();
                }
                self.friends.insert(friend.user, friend);
                vec![Area::Friends]
            }
            Update::FriendGone(user) => match self.friends.remove(&user) {
                Some(_) => vec![Area::Friends],
                None => Vec::new(),
            },
            Update::Message(message) => self.arrive(message),
            Update::Embeds {
                channel,
                message,
                embeds,
            } => self.on_timeline(&channel, |timeline| timeline.embed(&message, embeds)),
            Update::Deleted { channel, messages } => {
                self.on_timeline(&channel, |timeline| timeline.delete(&messages))
            }
            Update::Reaction(change) => self.on_timeline(&change.channel, |timeline| {
                timeline.react(&change.message, change.kind)
            }),
            Update::Typing { channel, user, at } => {
                if self.is_me(&user) {
                    return Vec::new();
                }
                let typers = self.typing.entry(channel).or_default();
                typers.retain(|(known, _)| known != &user);
                typers.push((user, at));
                vec![Area::Typing(channel)]
            }
            Update::Unread(unread) => {
                if self.unread.get(&unread.channel) == Some(&unread) {
                    return Vec::new();
                }
                let channel = unread.channel;
                let space = self.space_of(&channel);
                self.unread.insert(channel, unread);
                vec![Area::Unread(channel), Area::Unseen(space)]
            }
            Update::Voice(state) => self.put_voice(state),
            Update::VoiceGone { user, space } => match self.voice.remove(&(space, user)) {
                Some(left) => vec![Area::Voice(left.channel)],
                None => Vec::new(),
            },
            Update::Call(ringing) => {
                if self.calls.get(&ringing.channel) == Some(&ringing) {
                    return Vec::new();
                }
                self.calls.insert(ringing.channel, ringing);
                vec![Area::Calls]
            }
            Update::CallGone(channel) => match self.calls.remove(&channel) {
                Some(_) => vec![Area::Calls],
                None => Vec::new(),
            },
            Update::Roster { channel, roster } => {
                self.rosters.insert(channel, roster);
                vec![Area::Roster(channel)]
            }
            Update::Notify(_) => Vec::new(),
        }
    }

    fn ready(&mut self, ready: Ready) {
        *self = Store {
            link: self.link,
            ..Store::default()
        };
        self.own_presence = ready.own_presence;
        self.users.insert(ready.me.id, ready.me.clone());
        self.me = Some(ready.me);
        self.spaces = ready
            .spaces
            .into_iter()
            .map(|space| (space.id, space))
            .collect();
        self.folders = ready.folders;
        for (space, mut roles) in ready.roles {
            sort_roles(&mut roles);
            self.roles.insert(space, roles);
        }
        self.emojis = ready.emojis.into_iter().collect();
        self.stickers = ready.stickers.into_iter().collect();
        for channel in ready.channels {
            self.place(channel);
        }
        for user in ready.users {
            self.users.insert(user.id, user);
        }
        for member in ready.members {
            self.members
                .entry(member.space)
                .or_default()
                .insert(member.user, member);
        }
        self.presences = ready.presences.into_iter().collect();
        self.friends = ready
            .friends
            .into_iter()
            .map(|friend| (friend.user, friend))
            .collect();
        self.unread = ready
            .unread
            .into_iter()
            .map(|unread| (unread.channel, unread))
            .collect();
        for state in ready.voice {
            self.voice.insert((state.space, state.user), state);
        }
        self.calls = ready
            .calls
            .into_iter()
            .map(|ringing| (ringing.channel, ringing))
            .collect();
    }

    fn forget_space(&mut self, space: &SpaceId) -> Vec<Area> {
        self.spaces.remove(space);
        self.roles.remove(space);
        self.emojis.remove(space);
        self.stickers.remove(space);
        self.members.remove(space);
        let scope = Some(*space);
        let removed = self.places.remove(&scope).unwrap_or_default();
        let mut areas = vec![
            Area::Rail,
            Area::Space(*space),
            Area::Channels(scope),
            Area::Unseen(scope),
        ];
        for channel in removed {
            self.drop_channel_state(&channel);
            areas.push(Area::Channel(channel));
        }
        self.voice.retain(|(at, _), _| at.as_ref() != Some(space));
        areas
    }

    fn put_channel(&mut self, channel: Channel) -> Vec<Area> {
        let old = self.channels.get(&channel.id);
        if old == Some(&channel) {
            return Vec::new();
        }
        let muting = old.is_none_or(|old| old.muted != channel.muted);
        let id = channel.id;
        let space = channel.space;
        self.place(channel);
        let mut areas = vec![Area::Channel(id), Area::Channels(space)];
        if muting {
            areas.push(Area::Unseen(space));
        }
        areas
    }

    fn place(&mut self, channel: Channel) {
        if let Some(old) = self.channels.get(&channel.id)
            && old.space != channel.space
            && let Some(listed) = self.places.get_mut(&old.space)
        {
            listed.remove(&channel.id);
        }
        self.places
            .entry(channel.space)
            .or_default()
            .insert(channel.id);
        self.channels.insert(channel.id, channel);
    }

    fn space_of(&self, channel: &ChannelId) -> Option<SpaceId> {
        self.channels.get(channel).and_then(|known| known.space)
    }

    fn forget_channel(&mut self, channel: &ChannelId) -> Vec<Area> {
        let space = self.space_of(channel);
        self.drop_channel_state(channel);
        vec![
            Area::Channel(*channel),
            Area::Channels(space),
            Area::Unseen(space),
        ]
    }

    fn drop_channel_state(&mut self, channel: &ChannelId) {
        if let Some(removed) = self.channels.remove(channel)
            && let Some(listed) = self.places.get_mut(&removed.space)
        {
            listed.remove(channel);
        }
        self.unread.remove(channel);
        self.rosters.remove(channel);
        self.timelines.remove(channel);
        self.typing.remove(channel);
        self.visited.retain(|known| known != channel);
    }

    fn put_user(&mut self, user: User) -> Vec<Area> {
        if self.users.get(&user.id) == Some(&user) {
            return Vec::new();
        }
        let mut areas = vec![Area::User(user.id)];
        if self.is_me(&user.id) {
            self.me = Some(user.clone());
            areas.push(Area::Me);
        }
        self.users.insert(user.id, user);
        areas
    }

    fn adopt_users(&mut self, message: &Message) -> Vec<Area> {
        let mut areas = Vec::new();
        for user in std::iter::once(&message.author).chain(&message.mentioned) {
            if let Entry::Vacant(vacant) = self.users.entry(user.id) {
                vacant.insert(user.clone());
                areas.push(Area::User(user.id));
            }
        }
        areas
    }

    fn arrive(&mut self, message: Message) -> Vec<Area> {
        let channel = message.channel;
        let mut areas = self.adopt_users(&message);
        if let Some(typers) = self.typing.get_mut(&channel) {
            let before = typers.len();
            typers.retain(|(user, _)| user != &message.author.id);
            if typers.len() != before {
                areas.push(Area::Typing(channel));
            }
        }
        if message.delivery == Delivery::Delivered
            && let Some(known) = self.channels.get_mut(&channel)
            && known.last_activity.is_none_or(|last| last < message.at)
        {
            known.last_activity = Some(message.at);
            if known.kind.is_private() {
                areas.push(Area::Channels(known.space));
            }
        }
        if let Some(timeline) = self.timelines.get_mut(&channel)
            && timeline.arrive(message)
        {
            areas.push(Area::Timeline(channel));
        }
        areas
    }

    fn on_timeline(
        &mut self,
        channel: &ChannelId,
        change: impl FnOnce(&mut Timeline) -> bool,
    ) -> Vec<Area> {
        let changed = self.timelines.get_mut(channel).is_some_and(change);
        if changed {
            vec![Area::Timeline(*channel)]
        } else {
            Vec::new()
        }
    }

    fn put_voice(&mut self, state: VoiceState) -> Vec<Area> {
        let key = (state.space, state.user);
        if self.voice.get(&key) == Some(&state) {
            return Vec::new();
        }
        let mut areas = vec![Area::Voice(state.channel)];
        if let Some(old) = self.voice.insert(key, state)
            && areas.first() != Some(&Area::Voice(old.channel))
        {
            areas.push(Area::Voice(old.channel));
        }
        areas
    }

    pub fn absorb(&mut self, channel: &ChannelId, page: &Page, fetched: Vec<Message>) -> Vec<Area> {
        let mut areas: Vec<Area> = fetched
            .iter()
            .flat_map(|message| self.adopt_users(message))
            .collect();
        self.visit(channel);
        self.timelines
            .entry(*channel)
            .or_default()
            .absorb(page, fetched);
        areas.push(Area::Timeline(*channel));
        areas
    }

    pub fn visit(&mut self, channel: &ChannelId) {
        self.visited.retain(|known| known != channel);
        self.visited.push_front(*channel);
        while self.visited.len() > KEPT_TIMELINES {
            if let Some(oldest) = self.visited.pop_back() {
                self.timelines.remove(&oldest);
            }
        }
    }

    pub fn pend(&mut self, channel: &ChannelId, draft: &Draft) -> Area {
        let Some(me) = self.me.clone() else {
            return Area::Timeline(*channel);
        };
        let message = Message {
            id: draft.nonce,
            channel: *channel,
            author: me,
            at: draft.nonce.created_at(),
            edited: None,
            kind: MessageKind::Normal,
            text: draft.text.clone(),
            content: Rich::plain(draft.text.clone()),
            attachments: Vec::new(),
            embeds: Vec::new(),
            stickers: Vec::new(),
            reactions: Vec::new(),
            reply: draft.reply.as_ref().map(|reply| Reply {
                message: reply.message,
                channel: *channel,
                quoted: self
                    .timelines
                    .get(channel)
                    .and_then(|timeline| timeline.find(&reply.message))
                    .map(|quoted| {
                        Arc::new(Quoted {
                            author: quoted.author.clone(),
                            excerpt: quoted.content.clone(),
                            has_attachments: !quoted.attachments.is_empty(),
                            mentions_author: reply.mention,
                        })
                    }),
            }),
            mentioned: Vec::new(),
            mentions_me: false,
            pinned: false,
            thread: None,
            nonce: Some(draft.nonce),
            delivery: Delivery::Sending,
        };
        self.timelines.entry(*channel).or_default().pend(message);
        Area::Timeline(*channel)
    }

    pub fn fail(&mut self, channel: &ChannelId, nonce: &MessageId) -> Vec<Area> {
        self.on_timeline(channel, |timeline| timeline.fail(nonce))
    }

    pub fn withdraw(&mut self, channel: &ChannelId, nonce: &MessageId) -> Vec<Area> {
        self.on_timeline(channel, |timeline| timeline.withdraw(nonce))
    }

    pub fn expire_typing(&mut self, now: Instant) -> Vec<Area> {
        let mut areas = Vec::new();
        self.typing.retain(|channel, typers| {
            let before = typers.len();
            typers.retain(|(_, at)| now.saturating_duration_since(*at) < TYPING_LASTS);
            if typers.len() != before {
                areas.push(Area::Typing(*channel));
            }
            !typers.is_empty()
        });
        areas
    }

    #[must_use]
    pub fn link(&self) -> LinkState {
        self.link
    }

    #[must_use]
    pub fn me(&self) -> Option<&User> {
        self.me.as_ref()
    }

    #[must_use]
    pub fn is_me(&self, user: &UserId) -> bool {
        self.me.as_ref().is_some_and(|me| &me.id == user)
    }

    #[must_use]
    pub fn own_presence(&self) -> &Presence {
        &self.own_presence
    }

    #[must_use]
    pub fn space(&self, id: &SpaceId) -> Option<&Space> {
        self.spaces.get(id)
    }

    #[must_use]
    pub fn rail(&self) -> Vec<RailItem<'_>> {
        let mut placed: HashSet<&SpaceId> = HashSet::new();
        let mut items = Vec::new();
        for folder in &self.folders {
            let spaces: Vec<&Space> = folder
                .spaces
                .iter()
                .filter_map(|id| self.spaces.get(id))
                .collect();
            placed.extend(spaces.iter().map(|space| &space.id));
            match (folder.id.as_ref(), spaces.as_slice()) {
                (_, []) => {}
                (None, [space]) => items.push(RailItem::Space(space)),
                (None, many) => items.extend(many.iter().map(|space| RailItem::Space(space))),
                (Some(_), _) => items.push(RailItem::Folder { folder, spaces }),
            }
        }
        let mut loose: Vec<&Space> = self
            .spaces
            .values()
            .filter(|space| !placed.contains(&space.id))
            .collect();
        loose.sort_by_key(|space| Reverse(space.id));
        let mut rail: Vec<RailItem<'_>> = loose.into_iter().map(RailItem::Space).collect();
        rail.extend(items);
        rail
    }

    #[must_use]
    pub fn channel(&self, id: &ChannelId) -> Option<&Channel> {
        self.channels.get(id)
    }

    #[must_use]
    pub fn sections(&self, space: &SpaceId) -> Vec<Section<'_>> {
        layout::sections(self.placed(Some(*space)))
    }

    fn placed(&self, scope: Option<SpaceId>) -> impl Iterator<Item = &Channel> + '_ {
        self.places
            .get(&scope)
            .into_iter()
            .flatten()
            .filter_map(|id| self.channels.get(id))
    }

    #[must_use]
    pub fn directs(&self) -> Vec<&Channel> {
        let mut directs: Vec<&Channel> = self
            .placed(None)
            .filter(|channel| channel.kind.is_private())
            .collect();
        directs.sort_by(|left, right| {
            right
                .last_activity
                .cmp(&left.last_activity)
                .then_with(|| right.id.cmp(&left.id))
        });
        directs
    }

    #[must_use]
    pub fn direct_with(&self, user: &UserId) -> Option<&Channel> {
        self.placed(None).find(|channel| {
            channel.kind == ChannelKind::Direct && channel.recipients.first() == Some(user)
        })
    }

    #[must_use]
    pub fn channel_title(&self, channel: &Channel) -> String {
        if !channel.name.is_empty() {
            return channel.name.clone();
        }
        channel
            .recipients
            .iter()
            .map(|user| {
                self.users
                    .get(user)
                    .map_or_else(|| user.to_string(), |known| known.name().to_owned())
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    #[must_use]
    pub fn user(&self, id: &UserId) -> Option<&User> {
        self.users.get(id)
    }

    #[must_use]
    pub fn member(&self, space: &SpaceId, user: &UserId) -> Option<&Member> {
        self.members
            .get(space)
            .and_then(|members| members.get(user))
    }

    #[must_use]
    pub fn members(&self, space: &SpaceId) -> Vec<&Member> {
        self.members
            .get(space)
            .map(|members| members.values().collect())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn display_name(&self, space: Option<&SpaceId>, user: &User) -> String {
        space
            .and_then(|space| self.member(space, &user.id))
            .and_then(|member| member.nickname.clone())
            .filter(|nickname| !nickname.is_empty())
            .unwrap_or_else(|| user.name().to_owned())
    }

    #[must_use]
    pub fn name_of(&self, space: Option<&SpaceId>, user: &UserId) -> Option<String> {
        self.users
            .get(user)
            .map(|known| self.display_name(space, known))
    }

    #[must_use]
    pub fn roles(&self, space: &SpaceId) -> &[Role] {
        self.roles.get(space).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn role(&self, space: &SpaceId, id: &RoleId) -> Option<&Role> {
        self.roles(space).iter().find(|role| &role.id == id)
    }

    #[must_use]
    pub fn color_of(&self, space: Option<&SpaceId>, user: &UserId) -> Option<u32> {
        let space = space?;
        let member = self.member(space, user)?;
        self.roles(space)
            .iter()
            .filter(|role| member.roles.contains(&role.id))
            .find_map(|role| role.color)
    }

    #[must_use]
    pub fn emojis(&self, space: &SpaceId) -> &[Emoji] {
        self.emojis.get(space).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn stickers(&self, space: &SpaceId) -> &[Sticker] {
        self.stickers.get(space).map_or(&[], Vec::as_slice)
    }

    pub fn rail_spaces(&self) -> impl Iterator<Item = &Space> + '_ {
        self.rail().into_iter().flat_map(|item| match item {
            RailItem::Space(space) => vec![space],
            RailItem::Folder { spaces, .. } => spaces,
        })
    }

    #[must_use]
    pub fn presence(&self, user: &UserId) -> Option<&Presence> {
        if self.is_me(user) {
            return Some(&self.own_presence);
        }
        self.presences.get(user)
    }

    #[must_use]
    pub fn friends(&self) -> Vec<&Friend> {
        let mut friends: Vec<&Friend> = self.friends.values().collect();
        friends.sort_by_cached_key(|friend| {
            self.users
                .get(&friend.user)
                .map(|user| user.name().to_lowercase())
                .unwrap_or_default()
        });
        friends
    }
    #[must_use]
    pub fn bond(&self, user: &UserId) -> Option<Bond> {
        self.friends.get(user).map(|friend| friend.bond)
    }

    #[must_use]
    pub fn timeline(&self, channel: &ChannelId) -> Option<&Timeline> {
        self.timelines.get(channel)
    }

    #[must_use]
    pub fn typers(&self, channel: &ChannelId, now: Instant) -> Vec<&UserId> {
        self.typing
            .get(channel)
            .map(|typers| {
                typers
                    .iter()
                    .filter(|(_, at)| now.saturating_duration_since(*at) < TYPING_LASTS)
                    .map(|(user, _)| user)
                    .collect()
            })
            .unwrap_or_default()
    }

    #[must_use]
    pub fn unseen(&self, channel: &ChannelId) -> Unseen {
        self.unread
            .get(channel)
            .map_or_else(Unseen::default, |unread| Unseen {
                unread: unread.unread,
                mentions: unread.mentions,
            })
    }

    #[must_use]
    pub fn last_read(&self, channel: &ChannelId) -> Option<&MessageId> {
        self.unread
            .get(channel)
            .and_then(|unread| unread.last_read.as_ref())
    }

    #[must_use]
    pub fn space_unseen(&self, space: &SpaceId) -> Unseen {
        let muted_space = self.spaces.get(space).is_some_and(|known| known.muted);
        self.gather(Some(*space), muted_space)
    }

    #[must_use]
    pub fn directs_unseen(&self) -> Unseen {
        self.gather(None, false)
    }

    fn gather(&self, scope: Option<SpaceId>, muted_scope: bool) -> Unseen {
        let mut total = Unseen::default();
        for channel in self.placed(scope) {
            let Some(unread) = self.unread.get(&channel.id) else {
                continue;
            };
            let muted = muted_scope || channel.muted || self.parent_muted(channel);
            total.mentions += unread.mentions;
            total.unread |= unread.unread && !muted;
        }
        total
    }

    fn parent_muted(&self, channel: &Channel) -> bool {
        channel
            .parent
            .as_ref()
            .and_then(|parent| self.channels.get(parent))
            .is_some_and(|parent| parent.muted)
    }

    #[must_use]
    pub fn voice_in(&self, channel: &ChannelId) -> Vec<&VoiceState> {
        let mut states: Vec<&VoiceState> = self
            .voice
            .values()
            .filter(|state| &state.channel == channel)
            .collect();
        states.sort_by_cached_key(|state| {
            self.users
                .get(&state.user)
                .map(|user| user.name().to_lowercase())
                .unwrap_or_default()
        });
        states
    }

    #[must_use]
    pub fn call(&self, channel: &ChannelId) -> Option<&Ringing> {
        self.calls.get(channel)
    }

    #[must_use]
    pub fn incoming_calls(&self) -> Vec<&Ringing> {
        let Some(me) = self.me.as_ref() else {
            return Vec::new();
        };
        self.calls
            .values()
            .filter(|ringing| ringing.ringing.contains(&me.id))
            .collect()
    }

    #[must_use]
    pub fn my_voice(&self) -> Option<&VoiceState> {
        let me = &self.me.as_ref()?.id;
        self.voice.values().find(|state| &state.user == me)
    }

    #[must_use]
    pub fn roster(&self, channel: &ChannelId) -> Option<&Arc<Roster>> {
        self.rosters.get(channel)
    }
}

fn replaced<T: PartialEq>(
    values: &mut HashMap<SpaceId, T>,
    space: SpaceId,
    value: T,
    area: fn(SpaceId) -> Area,
) -> Vec<Area> {
    if values.get(&space) == Some(&value) {
        return Vec::new();
    }
    values.insert(space, value);
    vec![area(space)]
}

fn sort_roles(roles: &mut [Role]) {
    roles.sort_by(|left, right| {
        right
            .position
            .cmp(&left.position)
            .then_with(|| left.id.cmp(&right.id))
    });
}
