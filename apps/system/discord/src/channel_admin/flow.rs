use std::future::Future;

use discord_client::Client;
use discord_core::{ChannelEdit, ChannelId, ChannelSettings, Overwrite};
use rikka_app::prelude::*;

use super::{ChannelAdmin, Opened, Section};
use crate::chat::Chat;
use crate::space_admin::Edited;

fn edits(original: &ChannelSettings, draft: &ChannelSettings) -> Vec<ChannelEdit> {
    let mut edits = Vec::new();
    let mut push = |changed: bool, edit: ChannelEdit| {
        if changed {
            edits.push(edit);
        }
    };
    push(
        original.name != draft.name,
        ChannelEdit::Name(draft.name.clone()),
    );
    push(
        original.topic != draft.topic,
        ChannelEdit::Topic(draft.topic.clone()),
    );
    push(original.nsfw != draft.nsfw, ChannelEdit::Nsfw(draft.nsfw));
    push(
        original.slowmode != draft.slowmode,
        ChannelEdit::Slowmode(draft.slowmode),
    );
    push(
        original.bitrate != draft.bitrate,
        ChannelEdit::Bitrate(draft.bitrate),
    );
    push(
        original.user_limit != draft.user_limit,
        ChannelEdit::UserLimit(draft.user_limit),
    );
    push(
        original.region != draft.region,
        ChannelEdit::Region(draft.region.clone()),
    );
    push(
        original.video_quality != draft.video_quality,
        ChannelEdit::VideoQuality(draft.video_quality),
    );
    push(
        original.archive_minutes != draft.archive_minutes,
        ChannelEdit::ArchiveMinutes(draft.archive_minutes),
    );
    push(
        original.thread_slowmode != draft.thread_slowmode,
        ChannelEdit::ThreadSlowmode(draft.thread_slowmode),
    );
    edits
}

fn overwrite_changes(
    original: &ChannelSettings,
    draft: &ChannelSettings,
) -> (Vec<Overwrite>, Vec<Overwrite>) {
    let set: Vec<Overwrite> = draft
        .overwrites
        .iter()
        .filter(|overwrite| original.overwrite(overwrite.target) != Some(*overwrite))
        .copied()
        .collect();
    let removed: Vec<Overwrite> = original
        .overwrites
        .iter()
        .filter(|overwrite| draft.overwrite(overwrite.target).is_none())
        .copied()
        .collect();
    (set, removed)
}

impl ChannelAdmin {
    fn loaded(&mut self, settings: ChannelSettings) {
        self.saving.set(false);
        self.name.reset(&settings.name);
        self.topic.reset(&settings.topic);
        self.settings.set(Some(Ok(Edited::new(settings))));
    }

    fn channel_of(&self) -> Option<ChannelId> {
        self.opened.as_ref().map(|opened| opened.channel)
    }
}

impl Chat {
    pub(crate) fn open_channel_admin(&mut self, key: &str, channel: &ChannelId) {
        self.close_overlays();
        let admin = self.channel_admin_mut();
        *admin = ChannelAdmin::new();
        admin.opened.set(Some(Opened {
            key: key.to_owned(),
            channel: *channel,
        }));
        self.load_channel_settings();
    }

    pub(crate) fn forget_channel_admin(&mut self, channel: &ChannelId) {
        if self.channel_admin().channel_of() == Some(*channel) {
            *self.channel_admin_mut() = ChannelAdmin::new();
        }
    }

    pub(crate) fn close_channel_admin(&mut self) {
        let gone = self.channel_target().is_some_and(|(key, channel)| {
            self.linked(&key)
                .and_then(|linked| linked.channel(&channel))
                .is_none()
        });
        if self.channel_admin().is_dirty() && !gone {
            self.channel_admin_mut().warned.set(true);
            return;
        }
        *self.channel_admin_mut() = ChannelAdmin::new();
    }

    pub(super) fn show_channel_admin(&mut self, section: Section) {
        if self.channel_admin().is_dirty() {
            self.channel_admin_mut().warned.set(true);
            return;
        }
        self.channel_admin_mut().section.set(section);
        match section {
            Section::Invites => self.load_channel_invites(),
            Section::Integrations => self.load_channel_hooks(),
            Section::Overview | Section::Permissions | Section::Delete => {}
        }
    }

    pub(super) fn channel_target(&self) -> Option<(String, ChannelId)> {
        self.channel_admin()
            .opened
            .as_ref()
            .map(|opened| (opened.key.clone(), opened.channel))
    }

    pub(super) fn channel_admin_for(&mut self, channel: ChannelId) -> Option<&mut ChannelAdmin> {
        let admin = self.channel_admin_mut();
        (admin.channel_of() == Some(channel)).then_some(admin)
    }

    fn load_channel_settings(&mut self) {
        self.fetch_channel(
            |client, channel| async move { client.channel_settings(&channel).await },
            |admin, settings| match settings {
                Ok(settings) => admin.loaded(settings),
                Err(error_message) => admin.settings.set(Some(Err(error_message))),
            },
        );
    }

    pub(crate) fn reset_channel_admin(&mut self) {
        let admin = self.channel_admin_mut();
        admin.warned.set(false);
        if let Some(Ok(edited)) = (*admin.settings).clone() {
            admin.loaded(edited.original);
        }
    }

    pub(crate) fn save_channel_admin(&mut self) {
        let Some((key, channel)) = self.channel_target() else {
            return;
        };
        let admin = self.channel_admin();
        if *admin.saving {
            return;
        }
        let Some(Ok(edited)) = admin.settings.as_ref().cloned() else {
            return;
        };
        let changes = edits(&edited.original, &edited.draft);
        let (set, removed) = overwrite_changes(&edited.original, &edited.draft);
        self.channel_admin_mut().saving.set(true);
        self.attempt(
            &key,
            move |client| {
                let (changes, set, removed) = (changes.clone(), set.clone(), removed.clone());
                async move {
                    for overwrite in removed {
                        client.remove_overwrite(&channel, overwrite.target).await?;
                    }
                    for overwrite in set {
                        client.set_overwrite(&channel, overwrite).await?;
                    }
                    match changes.is_empty() {
                        true => client.channel_settings(&channel).await,
                        false => client.edit_channel(&channel, changes).await,
                    }
                }
            },
            move |chat: &mut Chat, saved: Option<ChannelSettings>| {
                if let Some(admin) = chat.channel_admin_for(channel) {
                    admin.saving.set(false);
                    admin.warned.set(false);
                    if let Some(saved) = saved {
                        admin.loaded(saved);
                    }
                }
            },
        );
    }

    pub(super) fn sync_channel_permissions(&mut self) {
        let Some((key, channel)) = self.channel_target() else {
            return;
        };
        let Some(parent) = self
            .channel_admin()
            .draft()
            .and_then(|settings| settings.parent)
        else {
            return;
        };
        self.act(
            &key,
            move |client| async move {
                client.sync_permissions(&channel, &parent).await?;
                client.channel_settings(&channel).await
            },
            move |chat: &mut Chat, settings: ChannelSettings| {
                if let Some(admin) = chat.channel_admin_for(channel) {
                    admin.loaded(settings);
                }
            },
        );
    }

    pub(super) fn load_channel_invites(&mut self) {
        self.fetch_channel(
            |client, channel| async move { client.channel_invites(&channel).await },
            |admin, invites| admin.invites.set(Some(invites)),
        );
    }

    pub(crate) fn refresh_channel_invites(&mut self) {
        if self.channel_admin().is_open() && *self.channel_admin().section == Section::Invites {
            self.load_channel_invites();
        }
    }

    pub(super) fn load_channel_hooks(&mut self) {
        self.fetch_channel(
            |client, channel| async move { client.channel_webhooks(&channel).await },
            |admin, hooks| admin.hooks.set(Some(hooks)),
        );
    }

    fn fetch_channel<T, F, Fut>(
        &mut self,
        task: F,
        store: impl Fn(&mut ChannelAdmin, Result<T, String>) + 'static,
    ) where
        T: Send + 'static,
        F: FnOnce(Client, ChannelId) -> Fut,
        Fut: Future<Output = discord_core::Result<T>> + Send + 'static,
    {
        let Some((key, channel)) = self.channel_target() else {
            return;
        };
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        spawn(
            task(client, channel),
            move |chat: &mut Chat, result: discord_core::Result<T>| {
                if let Some(admin) = chat.channel_admin_for(channel) {
                    store(admin, result.map_err(|error| error.to_string()));
                }
            },
        );
    }
}
