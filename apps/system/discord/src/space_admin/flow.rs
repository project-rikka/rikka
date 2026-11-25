use std::future::Future;

use discord_client::Client;
use discord_core::{SpaceEdit, SpaceId, SpaceSettings};
use rikka_app::prelude::*;

use super::{Edited, Images, Load, Opened, Section, SpaceAdmin};
use crate::chat::Chat;

impl SpaceAdmin {
    fn open(&mut self, opened: Opened, section: Section) {
        self.close();
        self.opened.set(Some(opened));
        self.section.set(section);
    }

    pub(super) fn space_of(&self) -> Option<SpaceId> {
        self.opened.as_ref().map(|opened| opened.space)
    }

    pub(super) fn warn(&mut self) {
        self.warned.set(true);
    }

    pub(super) fn is_warned(&self) -> bool {
        *self.warned
    }

    pub(super) fn is_saving(&self) -> bool {
        *self.saving
    }

    pub(super) fn settings_loaded(&mut self, settings: SpaceSettings) {
        self.saving.set(false);
        self.docs.name.reset(&settings.name);
        self.docs
            .description
            .reset(settings.description.as_deref().unwrap_or_default());
        self.images.set(Images::default());
        self.settings.set(Some(Ok(Edited::new(settings))));
    }

    fn settings_saved(&mut self, sent: &SpaceSettings, images: &Images, saved: SpaceSettings) {
        let Some(Ok(mut edited)) = (*self.settings).clone() else {
            return;
        };
        if edited.draft == *sent && *self.images == *images {
            self.settings_loaded(saved);
            return;
        }
        if *self.images == *images {
            self.images.set(Images::default());
        }
        edited.original = saved;
        self.settings.set(Some(Ok(edited)));
    }

    pub(super) fn settings_changed(
        &mut self,
        saved: SpaceSettings,
        apply: fn(&mut SpaceSettings, &SpaceSettings),
    ) {
        let Some(Ok(mut edited)) = (*self.settings).clone() else {
            return;
        };
        apply(&mut edited.draft, &saved);
        edited.original = saved;
        self.settings.set(Some(Ok(edited)));
    }

    fn reset(&mut self) {
        self.warned.set(false);
        match *self.section {
            Section::Overview | Section::Safety | Section::Community => {
                if let Some(Ok(edited)) = (*self.settings).clone() {
                    self.settings_loaded(edited.original);
                }
            }
            Section::Roles => self.reset_role(),
            Section::Widget => revert(&mut self.widget),
            Section::Vanity => {
                revert(&mut self.vanity);
                if let Some(Ok(edited)) = self.vanity.as_ref() {
                    let code = edited.original.code.clone().unwrap_or_default();
                    self.docs.vanity.reset(&code);
                }
            }
            Section::Welcome => {
                revert(&mut self.welcome);
                self.sync_welcome_docs();
            }
            Section::Onboarding => {
                revert(&mut self.onboarding);
                self.prompt_at.set(None);
            }
            _ => {}
        }
    }
}

fn revert<T: Clone + PartialEq>(load: &mut Tracked<super::Load<Edited<T>>>) {
    if let Some(Ok(edited)) = (**load).clone() {
        load.set(Some(Ok(Edited::new(edited.original))));
    }
}

fn settings_edits(
    original: &SpaceSettings,
    draft: &SpaceSettings,
    images: &Images,
) -> Vec<SpaceEdit> {
    let mut edits = Vec::new();
    let mut push = |changed: bool, edit: SpaceEdit| {
        if changed {
            edits.push(edit);
        }
    };
    push(
        original.name != draft.name,
        SpaceEdit::Name(draft.name.clone()),
    );
    push(
        original.description != draft.description,
        SpaceEdit::Description(draft.description.clone()),
    );
    push(
        original.verification != draft.verification,
        SpaceEdit::Verification(draft.verification),
    );
    push(
        original.content_filter != draft.content_filter,
        SpaceEdit::ContentFilter(draft.content_filter),
    );
    push(
        original.default_notifications != draft.default_notifications,
        SpaceEdit::DefaultNotifications(draft.default_notifications),
    );
    push(
        original.afk_channel != draft.afk_channel,
        SpaceEdit::AfkChannel(draft.afk_channel),
    );
    push(
        original.afk_timeout != draft.afk_timeout,
        SpaceEdit::AfkTimeout(draft.afk_timeout),
    );
    push(
        original.system_channel != draft.system_channel,
        SpaceEdit::SystemChannel(draft.system_channel),
    );
    push(
        original.system_flags != draft.system_flags,
        SpaceEdit::SystemFlags(draft.system_flags),
    );
    push(
        original.rules_channel != draft.rules_channel,
        SpaceEdit::RulesChannel(draft.rules_channel),
    );
    push(
        original.updates_channel != draft.updates_channel,
        SpaceEdit::UpdatesChannel(draft.updates_channel),
    );
    push(
        original.safety_channel != draft.safety_channel,
        SpaceEdit::SafetyChannel(draft.safety_channel),
    );
    push(
        original.locale != draft.locale,
        SpaceEdit::Locale(draft.locale.clone()),
    );
    push(
        original.progress_bar != draft.progress_bar,
        SpaceEdit::ProgressBar(draft.progress_bar),
    );
    push(
        original.features != draft.features,
        SpaceEdit::Features(draft.features.clone()),
    );
    if let Some(icon) = &images.icon {
        edits.push(SpaceEdit::Icon(icon.clone()));
    }
    if let Some(banner) = &images.banner {
        edits.push(SpaceEdit::Banner(banner.clone()));
    }
    if let Some(splash) = &images.splash {
        edits.push(SpaceEdit::Splash(splash.clone()));
    }
    edits
}

impl Chat {
    pub(crate) fn open_space_admin(&mut self, key: &str, space: &SpaceId, section: Section) {
        if let Some(link) = section.link(space) {
            self.open_page(&link);
            return;
        }
        self.close_overlays();
        self.space_admin_mut().open(
            Opened {
                key: key.to_owned(),
                space: *space,
            },
            section,
        );
        self.enter_admin_section(section);
    }

    pub(crate) fn show_space_admin(&mut self, section: Section) {
        let Some(space) = self.space_admin().space_of() else {
            return;
        };
        if let Some(link) = section.link(&space) {
            self.open_page(&link);
            return;
        }
        if self.space_admin().is_dirty() {
            self.space_admin_mut().warn();
            return;
        }
        let admin = self.space_admin_mut();
        admin.section.set(section);
        admin.warned.set(false);
        admin.upload.set(None);
        admin.hook.set(None);
        admin.rule.set(None);
        admin.renaming.set(None);
        self.enter_admin_section(section);
    }

    pub(crate) fn close_space_admin(&mut self) {
        let gone = self.admin_target().is_some_and(|(key, space)| {
            self.linked(&key)
                .and_then(|linked| linked.space(&space))
                .is_none()
        });
        if self.space_admin().is_dirty() && !gone {
            self.space_admin_mut().warn();
            return;
        }
        self.space_admin_mut().close();
    }

    pub(crate) fn admin_target(&self) -> Option<(String, SpaceId)> {
        self.space_admin()
            .opened()
            .map(|opened| (opened.key.clone(), opened.space))
    }

    pub(crate) fn admin_for(&mut self, space: SpaceId) -> Option<&mut SpaceAdmin> {
        let admin = self.space_admin_mut();
        (admin.space_of() == Some(space)).then_some(admin)
    }

    fn enter_admin_section(&mut self, section: Section) {
        match section {
            Section::Overview | Section::Safety | Section::Community => self.load_settings(),
            Section::Roles => self.load_role_counts(),
            Section::Emoji => self.load_emojis(),
            Section::Stickers => self.load_stickers(),
            Section::Soundboard => self.load_sounds(),
            Section::Widget => self.load_widget(),
            Section::Template => self.load_templates(),
            Section::Vanity => self.load_vanity(),
            Section::Integrations => {
                self.load_hooks();
                self.load_integrations();
            }
            Section::AutoMod => self.load_rules(),
            Section::Onboarding => self.load_onboarding(),
            Section::Welcome => self.load_welcome(),
            Section::Members => self.search_admin_members(),
            Section::Invites => self.load_invites(),
            Section::Bans => self.load_bans(),
            Section::AuditLog => self.load_audit(true),
            Section::Delete | Section::Boost | Section::Subscriptions | Section::Insights => {}
        }
    }

    pub(super) fn fetch<T, F, Fut>(
        &mut self,
        task: F,
        store: impl Fn(&mut SpaceAdmin, Result<T, String>) + 'static,
    ) where
        T: Send + 'static,
        F: FnOnce(Client, SpaceId) -> Fut,
        Fut: Future<Output = discord_core::Result<T>> + Send + 'static,
    {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let Some(client) = self.linked(&key).map(|linked| linked.client().clone()) else {
            return;
        };
        spawn(
            task(client, space),
            move |chat: &mut Chat, result: discord_core::Result<T>| {
                if let Some(admin) = chat.admin_for(space) {
                    store(admin, result.map_err(|error| error.to_string()));
                }
            },
        );
    }

    pub(crate) fn load_settings(&mut self) {
        if self.space_admin().settings.is_some() {
            return;
        }
        self.fetch(
            |client, space| async move { client.space_settings(&space).await },
            |admin, settings| match settings {
                Ok(settings) => admin.settings_loaded(settings),
                Err(error_message) => admin.settings.set(Some(Err(error_message))),
            },
        );
    }

    pub(crate) fn save_space_admin(&mut self) {
        if self.space_admin().is_saving() {
            return;
        }
        match self.space_admin().section() {
            Section::Overview | Section::Safety | Section::Community => self.save_settings(),
            Section::Roles => self.save_role(),
            Section::Widget => self.save_widget(),
            Section::Vanity => self.save_vanity(),
            Section::Welcome => self.save_welcome(),
            Section::Onboarding => self.save_onboarding(),
            _ => {}
        }
    }

    pub(crate) fn reset_space_admin(&mut self) {
        self.space_admin_mut().reset();
    }

    pub(super) fn admin_saved(&mut self, space: SpaceId) {
        if let Some(admin) = self.admin_for(space) {
            admin.saving.set(false);
            admin.warned.set(false);
        }
    }

    fn save_settings(&mut self) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let admin = self.space_admin();
        let Some(Ok(edited)) = admin.settings.as_ref() else {
            return;
        };
        let edits = settings_edits(&edited.original, &edited.draft, &admin.images);
        let mfa = (edited.original.mfa_required != edited.draft.mfa_required)
            .then_some(edited.draft.mfa_required);
        let (sent, images) = (edited.draft.clone(), (*admin.images).clone());
        self.space_admin_mut().saving.set(true);
        self.attempt(
            &key,
            move |client| {
                let edits = edits.clone();
                async move {
                    let edited = match edits.is_empty() {
                        true => None,
                        false => Some(client.edit_space(&space, edits).await?),
                    };
                    if let Some(required) = mfa {
                        client.require_mfa(&space, required).await?;
                    }
                    match (edited, mfa) {
                        (Some(edited), None) => Ok(edited),
                        _ => client.space_settings(&space).await,
                    }
                }
            },
            move |chat: &mut Chat, saved: Option<SpaceSettings>| {
                chat.admin_saved(space);
                if let (Some(saved), Some(admin)) = (saved, chat.admin_for(space)) {
                    admin.settings_saved(&sent, &images, saved);
                }
            },
        );
    }

    pub(super) fn save_edited<T, F, Fut>(
        &mut self,
        slot: fn(&mut SpaceAdmin) -> &mut Tracked<Load<Edited<T>>>,
        task: F,
    ) where
        T: Clone + PartialEq + Send + 'static,
        F: Fn(Client, SpaceId, T) -> Fut + Clone + 'static,
        Fut: Future<Output = discord_core::Result<()>> + Send + 'static,
    {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let Some(Ok(edited)) = (**slot(self.space_admin_mut())).clone() else {
            return;
        };
        let sent = edited.draft;
        self.space_admin_mut().saving.set(true);
        self.attempt(
            &key,
            move |client| {
                let sent = sent.clone();
                let saving = task(client, space, sent.clone());
                async move { saving.await.map(|()| sent) }
            },
            move |chat: &mut Chat, saved: Option<T>| {
                chat.admin_saved(space);
                if let (Some(saved), Some(admin)) = (saved, chat.admin_for(space)) {
                    mark_saved(slot(admin), saved);
                }
            },
        );
    }
}

fn mark_saved<T: Clone + PartialEq>(load: &mut Tracked<Load<Edited<T>>>, saved: T) {
    if let Some(Ok(mut edited)) = (**load).clone() {
        edited.original = saved;
        load.set(Some(Ok(edited)));
    }
}
