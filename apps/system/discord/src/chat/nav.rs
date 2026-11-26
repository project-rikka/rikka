use std::collections::BTreeSet;

use discord_core::{ChannelId, Focus, SpaceId};
use discord_preferences::ForumView;

use super::Chat;
use crate::account::{LinkedAccount, Location, Scope};

impl Chat {
    pub(crate) fn open_home(&mut self) {
        let Some(linked) = self.current() else {
            return;
        };
        let channel = linked.last_in(&Scope::Home).cloned();
        self.go(Location {
            scope: Scope::Home,
            channel,
        });
    }

    pub(crate) fn open_friends(&mut self) {
        self.go(Location::home());
    }

    pub(crate) fn open_space(&mut self, space: &SpaceId) {
        let Some(linked) = self.current() else {
            return;
        };
        let scope = Scope::Space(*space);
        let channel = linked
            .last_in(&scope)
            .filter(|channel| linked.channel(channel).is_some())
            .cloned()
            .or_else(|| first_channel(linked, space));
        self.go(Location { scope, channel });
    }

    pub(crate) fn open_channel(&mut self, channel: &ChannelId) {
        let Some(channel_info) = self.current().and_then(|linked| linked.channel(channel)) else {
            return;
        };
        let scope = match &channel_info.space {
            Some(space) => Scope::Space(*space),
            None => Scope::Home,
        };
        self.go(Location {
            scope,
            channel: Some(*channel),
        });
    }

    pub(crate) fn toggle_folder(&mut self, folder: &str) {
        if let Some(linked) = self.current_mut() {
            linked.toggle_folder(folder);
        }
    }

    pub(crate) fn collapsed_categories(&self) -> BTreeSet<ChannelId> {
        self.current_key()
            .map(|key| self.preferences.collapsed_in(key))
            .unwrap_or_default()
    }

    fn is_collapsed(&self, category: &ChannelId) -> bool {
        self.collapsed_categories().contains(category)
    }

    pub(crate) fn toggle_category(&mut self, category: &ChannelId) {
        let Some(key) = (*self.current).clone() else {
            return;
        };
        let collapsed = self.is_collapsed(category);
        self.preferences.set_collapsed(&key, category, !collapsed);
    }

    pub(crate) fn forum_view(&self, key: &str, forum: &ChannelId) -> ForumView {
        self.preferences.forum_view(key, forum)
    }

    pub(crate) fn remember_forum_view(&mut self, key: &str, forum: &ChannelId, view: ForumView) {
        self.preferences.set_forum_view(key, forum, view);
    }

    pub(crate) fn open_dm(&mut self, channel: ChannelId) {
        self.go(Location {
            scope: Scope::Home,
            channel: Some(channel),
        });
    }

    pub(crate) fn step_channel(&mut self, by: isize) {
        let Some(linked) = self.current() else {
            return;
        };
        let location = linked.location();
        let order: Vec<ChannelId> = match &location.scope {
            Scope::Home => linked.directs().iter().map(|direct| direct.id).collect(),
            Scope::Space(space) => linked
                .sections(space)
                .iter()
                .flat_map(|section| section.rows.iter())
                .flat_map(|row| std::iter::once(row.channel).chain(row.threads.iter().copied()))
                .filter(|channel| channel.kind.holds_messages() && !channel.kind.is_voice())
                .map(|channel| channel.id)
                .collect(),
        };
        if order.is_empty() {
            return;
        }
        let at = location
            .channel
            .as_ref()
            .and_then(|channel| order.iter().position(|known| known == channel));
        let next = match at {
            Some(at) => (at as isize + by).rem_euclid(order.len() as isize) as usize,
            None if by < 0 => order.len() - 1,
            None => 0,
        };
        let channel = order[next];
        self.open_channel(&channel);
    }

    fn go(&mut self, location: Location) {
        self.close_overlays();
        let Some(linked) = self.current_mut() else {
            return;
        };
        if linked.go(location) {
            self.arrive();
        }
    }

    pub(crate) fn arrive(&mut self) {
        let Some(linked) = self.current() else {
            return;
        };
        let location = linked.location().clone();
        linked.client().focus(Focus {
            space: location.scope.space().cloned(),
            channel: location.channel,
        });
        self.entered();
    }
}

fn first_channel(linked: &LinkedAccount, space: &SpaceId) -> Option<ChannelId> {
    linked
        .sections(space)
        .iter()
        .flat_map(|section| section.rows.iter())
        .map(|row| row.channel)
        .find(|channel| channel.kind.holds_messages() && !channel.kind.is_voice())
        .map(|channel| channel.id)
}
