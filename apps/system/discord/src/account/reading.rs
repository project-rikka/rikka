use std::sync::Arc;
use std::time::Instant;

use discord_core::{
    Area, Bond, Channel, ChannelId, Emoji, Folder, Friend, LinkState, Member, MessageId, Presence,
    RailItem, Ringing, Role, RoleId, Roster, Section, Space, SpaceId, Sticker, Store, Timeline,
    Unseen, User, UserId, VoiceState,
};

use super::LinkedAccount;

impl LinkedAccount {
    pub(crate) fn watching(&self, areas: impl IntoIterator<Item = Area>) -> &Store {
        for area in areas {
            self.marks.read(area);
        }
        &self.store
    }

    fn watching_users<'a>(&self, users: impl IntoIterator<Item = &'a UserId>) {
        for user in users {
            self.marks.read(Area::User(*user));
        }
    }

    pub(crate) fn link(&self) -> LinkState {
        self.watching([Area::Link]).link()
    }

    pub(crate) fn me(&self) -> Option<&User> {
        self.watching([Area::Me]).me()
    }

    pub(crate) fn own_presence(&self) -> &Presence {
        if let Some(me) = self.me() {
            self.marks.read(Area::Presence(me.id));
        }
        self.store.own_presence()
    }

    pub(crate) fn space(&self, id: &SpaceId) -> Option<&Space> {
        self.watching([Area::Rail, Area::Space(*id)]).space(id)
    }

    pub(crate) fn rail(&self) -> Vec<RailItem<'_>> {
        self.watching([Area::Rail]).rail()
    }

    pub(crate) fn folder(&self, id: &str) -> Option<&Folder> {
        self.rail().into_iter().find_map(|item| match item {
            RailItem::Folder { folder, .. } if folder.id.as_deref() == Some(id) => Some(folder),
            RailItem::Folder { .. } | RailItem::Space(_) => None,
        })
    }

    pub(crate) fn top_role_position(&self, space: &SpaceId, user: &UserId) -> i32 {
        self.member(space, user).map_or(0, |member| {
            member
                .roles
                .iter()
                .filter_map(|role| self.role(space, role))
                .map(|role| role.position)
                .max()
                .unwrap_or(0)
        })
    }

    pub(crate) fn outranks(&self, space: &SpaceId, user: &UserId) -> bool {
        let (Some(known), Some(me)) = (self.space(space), self.me().map(|me| me.id)) else {
            return false;
        };
        !known.owned_by(user)
            && (known.owned_by(&me)
                || self.top_role_position(space, &me) > self.top_role_position(space, user))
    }

    pub(crate) fn channel(&self, id: &ChannelId) -> Option<&Channel> {
        self.watching([Area::Channel(*id)]).channel(id)
    }

    pub(crate) fn sections(&self, space: &SpaceId) -> Vec<Section<'_>> {
        self.watching([Area::Channels(Some(*space))])
            .sections(space)
    }

    pub(crate) fn directs(&self) -> Vec<&Channel> {
        self.watching([Area::Channels(None)]).directs()
    }

    pub(crate) fn direct_with(&self, user: &UserId) -> Option<&Channel> {
        self.watching([Area::Channels(None)]).direct_with(user)
    }

    pub(crate) fn channel_title(&self, channel: &Channel) -> String {
        self.watching_users(&channel.recipients);
        self.store.channel_title(channel)
    }

    pub(crate) fn user(&self, id: &UserId) -> Option<&User> {
        self.watching([Area::User(*id)]).user(id)
    }

    pub(crate) fn display_name(&self, space: Option<&SpaceId>, user: &User) -> String {
        self.watching(space.map(|space| Area::Member(*space, user.id)))
            .display_name(space, user)
    }

    pub(crate) fn name_of(&self, space: Option<&SpaceId>, user: &UserId) -> Option<String> {
        let member = space.map(|space| Area::Member(*space, *user));
        self.watching(std::iter::once(Area::User(*user)).chain(member))
            .name_of(space, user)
    }

    pub(crate) fn color_of(&self, space: Option<&SpaceId>, user: &UserId) -> Option<u32> {
        let areas = space
            .into_iter()
            .flat_map(|space| [Area::Space(*space), Area::Member(*space, *user)]);
        self.watching(areas).color_of(space, user)
    }

    pub(crate) fn presence(&self, user: &UserId) -> Option<&Presence> {
        self.watching([Area::Presence(*user)]).presence(user)
    }

    pub(crate) fn timeline(&self, channel: &ChannelId) -> Option<&Timeline> {
        self.watching([Area::Timeline(*channel)]).timeline(channel)
    }

    pub(crate) fn unseen(&self, channel: &ChannelId) -> Unseen {
        self.watching([Area::Unread(*channel)]).unseen(channel)
    }

    pub(crate) fn space_unseen(&self, space: &SpaceId) -> Unseen {
        self.watching([Area::Unseen(Some(*space))])
            .space_unseen(space)
    }

    pub(crate) fn directs_unseen(&self) -> Unseen {
        self.watching([Area::Unseen(None)]).directs_unseen()
    }

    pub(crate) fn voice_in(&self, channel: &ChannelId) -> Vec<&VoiceState> {
        let states = self.watching([Area::Voice(*channel)]).voice_in(channel);
        self.watching_users(states.iter().map(|state| &state.user));
        states
    }

    pub(crate) fn is_me(&self, user: &UserId) -> bool {
        self.watching([Area::Me]).is_me(user)
    }

    pub(crate) fn member(&self, space: &SpaceId, user: &UserId) -> Option<&Member> {
        self.watching([Area::Member(*space, *user)])
            .member(space, user)
    }

    pub(crate) fn role(&self, space: &SpaceId, id: &RoleId) -> Option<&Role> {
        self.watching([Area::Space(*space)]).role(space, id)
    }

    pub(crate) fn friends(&self) -> Vec<&Friend> {
        let friends = self.watching([Area::Friends]).friends();
        self.watching_users(friends.iter().map(|friend| &friend.user));
        friends
    }
    pub(crate) fn bond(&self, user: &UserId) -> Option<Bond> {
        self.watching([Area::Friends]).bond(user)
    }

    pub(crate) fn roster(&self, channel: &ChannelId) -> Option<&Arc<Roster>> {
        self.watching([Area::Roster(*channel)]).roster(channel)
    }

    pub(crate) fn incoming_calls(&self) -> Vec<&Ringing> {
        self.watching([Area::Calls, Area::Me]).incoming_calls()
    }

    pub(crate) fn typers(&self, channel: &ChannelId, now: Instant) -> Vec<&UserId> {
        self.watching([Area::Typing(*channel)]).typers(channel, now)
    }

    pub(crate) fn last_read(&self, channel: &ChannelId) -> Option<&MessageId> {
        self.watching([Area::Unread(*channel)]).last_read(channel)
    }

    pub(crate) fn roles(&self, space: &SpaceId) -> &[Role] {
        self.watching([Area::Space(*space)]).roles(space)
    }

    pub(crate) fn members(&self, space: &SpaceId) -> Vec<&Member> {
        self.watching([Area::Members(*space)]).members(space)
    }

    pub(crate) fn emoji_spaces(&self) -> Vec<(&Space, &[Emoji])> {
        self.per_space(Store::emojis)
    }

    pub(crate) fn sticker_spaces(&self) -> Vec<(&Space, &[Sticker])> {
        self.per_space(Store::stickers)
    }

    fn per_space<'a, T>(
        &'a self,
        items_of: impl Fn(&'a Store, &SpaceId) -> &'a [T],
    ) -> Vec<(&'a Space, &'a [T])> {
        let store = self.watching([Area::Rail]);
        store
            .rail_spaces()
            .filter_map(|space| {
                self.marks.read(Area::Space(space.id));
                let items = items_of(store, &space.id);
                (!items.is_empty()).then_some((space, items))
            })
            .collect()
    }
}
