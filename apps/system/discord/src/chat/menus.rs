use discord_core::{
    ChannelId, ChannelKind, MuteSpan, NotifyChange, NotifyLevel, Permissions, SpaceId,
};
use rikka_app::prelude::*;

use super::Chat;
use crate::menus::{Menus, Shown, Subject};

impl Chat {
    pub(crate) fn menus(&self) -> &Menus {
        &self.menus
    }

    pub(crate) fn open_context_menu(&mut self, subject: Subject, anchor: Anchor) {
        let Some(key) = self.current_key().map(str::to_owned) else {
            return;
        };
        self.close_overlays();
        self.menus.show(Shown {
            key,
            subject,
            anchor,
        });
    }

    pub(crate) fn mark_space_read(&mut self, key: &str, space: &SpaceId) {
        self.mark_spaces_read(key, vec![*space]);
    }

    pub(crate) fn mark_folder_read(&mut self, key: &str, folder: &str) {
        let Some(spaces) = self
            .linked(key)
            .and_then(|linked| linked.folder(folder))
            .map(|found| found.spaces.clone())
        else {
            return;
        };
        self.mark_spaces_read(key, spaces);
    }

    fn mark_spaces_read(&mut self, key: &str, spaces: Vec<SpaceId>) {
        self.act(
            key,
            move |client| {
                let spaces = spaces.clone();
                async move { client.mark_spaces_read(&spaces).await }
            },
            |_: &mut Chat, ()| {},
        );
    }

    pub(crate) fn mark_beneath_read(&mut self, key: &str, channel: &ChannelId) {
        let channel = *channel;
        self.act(
            key,
            move |client| async move { client.mark_beneath_read(&channel).await },
            |_: &mut Chat, ()| {},
        );
    }

    pub(crate) fn invite_to_space(&mut self, key: &str, space: &SpaceId) {
        let Some(linked) = self.linked(key) else {
            return;
        };
        let invitable = |channel: &ChannelId| {
            linked.channel(channel).is_some_and(|known| {
                known.space == Some(*space)
                    && known.kind.holds_messages()
                    && known.kind != ChannelKind::Thread
                    && known
                        .permissions
                        .contains(Permissions::CREATE_INSTANT_INVITE)
            })
        };
        let here = linked
            .location()
            .channel
            .filter(|channel| invitable(channel));
        let chosen = here.or_else(|| {
            linked
                .sections(space)
                .iter()
                .flat_map(|section| section.rows.iter())
                .map(|row| row.channel.id)
                .find(|channel| invitable(channel))
        });
        match chosen {
            Some(channel) => self.open_invite(key, &channel),
            None => self
                .manage
                .failed(tr!("discord.manage.no-invite-channel").to_string()),
        }
    }

    pub(crate) fn mute_space(&mut self, key: &str, space: &SpaceId, span: Option<MuteSpan>) {
        let space = *space;
        self.act(
            key,
            move |client| async move { client.mute_space(&space, span).await },
            |_: &mut Chat, ()| {},
        );
    }

    pub(crate) fn change_space_notify(&mut self, key: &str, space: &SpaceId, change: NotifyChange) {
        let space = *space;
        self.act(
            key,
            move |client| async move { client.change_space_notify(&space, change).await },
            |_: &mut Chat, ()| {},
        );
    }

    pub(crate) fn mute_channel(
        &mut self,
        key: &str,
        space: &SpaceId,
        channel: &ChannelId,
        span: Option<MuteSpan>,
    ) {
        let (space, channel) = (*space, *channel);
        self.act(
            key,
            move |client| async move { client.mute_channel(&space, &channel, span).await },
            |_: &mut Chat, ()| {},
        );
    }

    pub(crate) fn set_channel_level(
        &mut self,
        key: &str,
        space: &SpaceId,
        channel: &ChannelId,
        level: Option<NotifyLevel>,
    ) {
        let (space, channel) = (*space, *channel);
        self.act(
            key,
            move |client| async move { client.set_channel_level(&space, &channel, level).await },
            |_: &mut Chat, ()| {},
        );
    }

    pub(crate) fn clone_channel(&mut self, key: &str, channel: &ChannelId) {
        let channel = *channel;
        self.act(
            key,
            move |client| async move { client.clone_channel(&channel).await },
            |_: &mut Chat, _: ChannelId| {},
        );
    }

    pub(crate) fn collapse_all_categories(&mut self, space: &SpaceId) {
        let Some(key) = self.current_key().map(str::to_owned) else {
            return;
        };
        let Some(categories) = self.linked(&key).map(|linked| {
            linked
                .sections(space)
                .iter()
                .filter_map(|section| section.category.map(|category| category.id))
                .collect::<Vec<_>>()
        }) else {
            return;
        };
        let collapsed = self.collapsed_categories();
        for category in categories
            .iter()
            .filter(|category| !collapsed.contains(category))
        {
            self.toggle_category(category);
        }
    }

    pub(crate) fn open_page(&mut self, url: &str) {
        if let Err(error) = rikka_open::open(url) {
            self.manage.failed(
                tr!(
                    "discord.manage.open-failed",
                    reason = error.to_string().as_str()
                )
                .to_string(),
            );
        }
    }
}
