use discord_client::Client;
use discord_core::{ChannelId, SpaceId, UserId};
use rikka_app::prelude::*;

use super::Chat;
use crate::member_menu::MemberTarget;
use crate::profile::{ProfileTarget, Profiles, Tab};

const DIRECT: &str = "discord-open-direct";
const CLOSE: &str = "discord-close-direct";
const NOTE: &str = "discord-note";

impl Chat {
    pub(crate) fn profiles(&self) -> &Profiles {
        &self.profiles
    }

    pub(crate) fn open_profile(&mut self, user: &UserId, space: Option<&SpaceId>, anchor: Anchor) {
        let Some(key) = self.current_key().map(str::to_owned) else {
            return;
        };
        self.member_menu.close();
        self.close_full_profile();
        let target = ProfileTarget {
            key,
            user: *user,
            space: space.cloned(),
        };
        self.profiles.show_popout(target, anchor);
    }

    pub(crate) fn open_full_profile(&mut self, target: &ProfileTarget, tab: Tab) {
        self.member_menu.close();
        if self
            .profiles
            .full()
            .is_some_and(|(shown, _)| shown == target)
        {
            self.profiles.show_tab(tab);
            return;
        }
        self.save_note();
        self.profiles.show_full(target.clone(), tab);
        let Some(linked) = self.linked(&target.key) else {
            return;
        };
        if linked.is_me(&target.user) {
            return;
        }
        let client = linked.client().clone();
        let (user_id, landed) = (target.user, target.clone());
        spawn_keyed(
            (NOTE, target.key.clone(), target.user),
            async move { client.note(&user_id).await },
            move |profiles: &mut Profiles, note: discord_core::Result<String>| match note {
                Ok(note) => profiles.on_note_loaded(&landed, note),
                Err(error) => tracing::warn!("failed to read note: {error}"),
            },
        );
    }

    pub(crate) fn close_full_profile(&mut self) {
        self.save_note();
        self.profiles.close_full();
    }

    pub(crate) fn close_profiles(&mut self) {
        self.profiles.close_popout();
        self.close_full_profile();
    }

    pub(crate) fn save_note(&mut self) {
        let Some((target, _)) = self.profiles.full().cloned() else {
            return;
        };
        let Some(note) = self.profiles.changed_note() else {
            return;
        };
        let Some(client) = self
            .linked(&target.key)
            .map(|linked| linked.client().clone())
        else {
            return;
        };
        spawn(
            async move { client.set_note(&target.user, &note).await },
            |chat: &mut Chat, saved: discord_core::Result<()>| {
                if let Err(error) = saved {
                    chat.error_message.set(Some(error.to_string()));
                }
            },
        );
    }

    pub(crate) fn open_profile_menu(&mut self, target: &ProfileTarget, anchor: Anchor) {
        self.member_menu.show(MemberTarget {
            key: target.key.clone(),
            user: target.user,
            space: target.space,
            anchor,
            over_profile: true,
        });
    }

    pub(crate) fn open_mutual_space(&mut self, space: &SpaceId) {
        self.close_profiles();
        self.open_space(space);
    }

    pub(crate) fn send_from_profile(&mut self) {
        let Some((target, _)) = self.profiles.popout().cloned() else {
            return;
        };
        let caption = self.profiles.take_message();
        if caption.is_empty() {
            skip_view();
            return;
        }
        self.open_direct_message(&target.key, &target.user, Some(caption));
    }

    pub(crate) fn add_friend(&mut self, target: &ProfileTarget) {
        let Some(client) = self
            .linked(&target.key)
            .map(|linked| linked.client().clone())
        else {
            return;
        };
        let user = target.user;
        spawn(
            async move { client.request_friend(&user).await },
            |chat: &mut Chat, sent: discord_core::Result<()>| {
                if let Err(error) = sent {
                    chat.error_message.set(Some(error.to_string()));
                }
            },
        );
    }

    pub(crate) fn message_user(&mut self, key: &str, user: &UserId) {
        self.open_direct_message(key, user, None);
    }

    fn open_direct_message(&mut self, key: &str, user: &UserId, caption: Option<String>) {
        self.close_profiles();
        self.member_menu.close();
        if self.current_key() != Some(key) {
            self.switch(key);
        }
        let Some(linked) = self.linked(key) else {
            return;
        };
        if let Some(direct) = linked.direct_with(user).map(|direct| direct.id) {
            self.open_channel(&direct);
            if let Some(caption) = caption {
                self.say(key, &direct, caption);
            }
            return;
        }
        let client = linked.client().clone();
        let (user_id, account_key) = (*user, key.to_owned());
        spawn_keyed(
            (DIRECT, key.to_owned(), *user),
            async move { client.open_direct(&user_id).await },
            move |chat: &mut Chat, channel: discord_core::Result<ChannelId>| match channel {
                Ok(channel) if chat.current_key() == Some(account_key.as_str()) => {
                    chat.open_dm(channel);
                    if let Some(caption) = caption.clone() {
                        chat.say(&account_key, &channel, caption);
                    }
                }
                Ok(_) => {}
                Err(error) => chat.error_message.set(Some(error.to_string())),
            },
        );
    }
    pub(crate) fn close_direct(&mut self, channel: &ChannelId) {
        let Some(linked) = self.current() else {
            return;
        };
        let client = linked.client().clone();
        let channel_id = *channel;
        if linked.location().channel.as_ref() == Some(channel) {
            self.open_friends();
        }
        spawn_keyed(
            (CLOSE, *channel),
            async move { client.close_direct(&channel_id).await },
            |chat: &mut Chat, result: discord_core::Result<()>| {
                if let Err(error) = result {
                    chat.error_message.set(Some(error.to_string()));
                }
            },
        );
    }

    pub(crate) fn add_friend_by_name(&mut self) {
        let handle = self.friends.handle();
        if handle.is_empty() {
            return;
        }
        self.befriending(move |client| async move { client.request_friend_by_name(&handle).await });
    }

    pub(crate) fn accept_friend(&mut self, user: &UserId) {
        let user = *user;
        self.befriending(move |client| async move { client.accept_friend(&user).await });
    }

    pub(crate) fn unfriend(&mut self, user: &UserId) {
        let user = *user;
        self.befriending(move |client| async move { client.unfriend(&user).await });
    }

    fn befriending<F>(&mut self, ask: impl FnOnce(Client) -> F)
    where
        F: Future<Output = discord_core::Result<()>> + Send + 'static,
    {
        let Some(client) = self.current().map(|linked| linked.client().clone()) else {
            return;
        };
        spawn(
            ask(client),
            |chat: &mut Chat, result: discord_core::Result<()>| {
                chat.friends
                    .answered(result.map_err(|error| error.to_string()));
            },
        );
    }
}
