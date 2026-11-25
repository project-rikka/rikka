use discord_core::{ChannelId, ImageUpload, Integration, SpaceId, Webhook};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::overview::{channel_combo, channels};
use super::{SpaceAdmin, hint, loading};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::manage::{Prompt, pick_icon};
use crate::menus::APP_DIRECTORY;
use crate::widgets::avatar;

const LONGEST_NAME: usize = 80;

pub(super) fn view(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let hooks = loading(&admin.hooks, |hooks| {
        column_of(hooks.iter().filter(|hook| hook.incoming).map(|hook| {
            match admin.hook.as_ref().filter(|edited| edited.id == hook.id) {
                Some(edited) => hook_editor(admin, linked, space, edited),
                None => hook_row(linked, hook),
            }
        }))
        .gap(Space::SNUG)
        .fill_width()
        .any()
    });
    let integrations = loading(&admin.integrations, |integrations| {
        column_of(integrations.iter().map(integration_row))
            .gap(Space::SNUG)
            .fill_width()
            .any()
    });
    column! {
        section(
            tr!("discord.admin.webhooks"),
            column! {
                hint(tr!("discord.admin.webhooks-hint"));
                button(text(tr!("discord.admin.new-webhook")))
                    .primary()
                    .small()
                    .on_press(|chat: &mut Chat| chat.create_webhook());
                hooks;
            }
            .gap(Space::SNUG)
            .fill_width(),
        );
        section(
            tr!("discord.admin.apps"),
            column! {
                integrations;
                button(text(tr!("discord.menu.app-directory")))
                    .secondary()
                    .small()
                    .on_press(|chat: &mut Chat| chat.open_page(APP_DIRECTORY));
            }
            .gap(Space::SNUG)
            .fill_width(),
        );
    }
    .gap(Space::LOOSE)
    .fill_width()
    .any()
}

fn hook_row(linked: &LinkedAccount, hook: &Webhook) -> Element {
    let channel = hook
        .channel
        .and_then(|channel| linked.channel(&channel))
        .map(|channel| format!("# {}", channel.name))
        .unwrap_or_default();
    let edited = hook.clone();
    card(
        row! {
            avatar(hook.avatar.as_ref(), &hook.name, 36.0);
            column! {
                text(hook.name.clone()).label().semibold().no_wrap();
                text(channel).caption().muted().no_wrap();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            button(text(tr!("discord.admin.edit")))
                .ghost()
                .small()
                .on_press(move |admin: &mut SpaceAdmin| admin.edit_hook(&edited));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .any()
}

fn hook_editor(
    admin: &SpaceAdmin,
    linked: &LinkedAccount,
    space: &SpaceId,
    hook: &Webhook,
) -> Element {
    let url = hook.url.clone().unwrap_or_default();
    let id = hook.id;
    card(
        column! {
            row! {
                avatar(hook.avatar.as_ref(), &hook.name, 48.0);
                button(text(tr!("discord.admin.upload")))
                    .secondary()
                    .small()
                    .on_press(move |chat: &mut Chat| chat.pick_hook_avatar());
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
            text(tr!("discord.admin.webhook-name")).caption().semibold().muted();
            text_field(&admin.docs.hook_name).max_len(LONGEST_NAME).fill_width();
            field(
                tr!("discord.admin.webhook-channel"),
                channel_combo(hook.channel, channels(linked, space, false), None, |admin, chosen| {
                    if let Some(mut hook) = (*admin.hook).clone() {
                        hook.channel = chosen;
                        admin.hook.set(Some(hook));
                    }
                }),
            );
            row! {
                button(text(tr!("discord.admin.webhook-copy-url")))
                    .secondary()
                    .small()
                    .enabled(!url.is_empty())
                    .on_press(move || {
                        set_text_of(Selection::Clipboard, &url);
                        skip_view();
                    });
                button(text(tr!("discord.admin.webhook-delete")))
                    .danger()
                    .small()
                    .on_press(move |chat: &mut Chat| chat.ask_delete_hook(&id));
                spacer().grow(1.0);
                button(text(tr!("discord.manage.cancel")))
                    .ghost()
                    .small()
                    .on_press(|admin: &mut SpaceAdmin| admin.hook.set(None));
                button(text(tr!("discord.manage.save")))
                    .primary()
                    .small()
                    .on_press(|chat: &mut Chat| chat.save_hook(None));
            }
            .gap(Space::SNUG)
            .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .any()
}

fn integration_row(integration: &Integration) -> Element {
    let id = integration.id;
    let kind = match integration.application.as_deref() {
        Some(application) if application != integration.name => {
            format!("{} · {}", integration.kind, application)
        }
        _ => integration.kind.clone(),
    };
    card(
        row! {
            avatar(integration.icon.as_ref(), &integration.name, 36.0);
            column! {
                text(integration.name.clone()).label().semibold().no_wrap();
                text(kind).caption().muted().no_wrap();
                if let Some(creator) = &integration.creator {
                    text(tr!("discord.admin.added-by", name = creator.name())).caption().muted().no_wrap();
                }
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            button(text(tr!("discord.admin.remove")))
                .danger()
                .small()
                .on_press(move |chat: &mut Chat| chat.ask_remove_integration(&id));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .any()
}

impl SpaceAdmin {
    fn edit_hook(&mut self, hook: &Webhook) {
        self.docs.hook_name.reset(&hook.name);
        self.hook.set(Some(hook.clone()));
    }

    fn change_hooks(&mut self, change: impl FnOnce(&mut Vec<Webhook>)) {
        if let Some(Ok(mut hooks)) = (*self.hooks).clone() {
            change(&mut hooks);
            self.hooks.set(Some(Ok(hooks)));
        }
    }
}

impl Chat {
    pub(super) fn load_hooks(&mut self) {
        self.fetch(
            |client, space| async move { client.space_webhooks(&space).await },
            |admin, hooks| admin.hooks.set(Some(hooks)),
        );
    }

    pub(super) fn load_integrations(&mut self) {
        self.fetch(
            |client, space| async move { client.integrations(&space).await },
            |admin, integrations| admin.integrations.set(Some(integrations)),
        );
    }

    fn create_webhook(&mut self) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let Some(channel) = self.linked(&key).and_then(|linked| {
            super::overview::channels(linked, &space, false)
                .first()
                .map(|(id, _)| *id)
        }) else {
            return;
        };
        let name = tr!("discord.admin.webhook-default-name").to_string();
        self.act(
            &key,
            move |client| {
                let name = name.clone();
                async move { client.create_webhook(&channel, &name).await }
            },
            move |chat: &mut Chat, hook: Webhook| {
                if let Some(admin) = chat.admin_for(space) {
                    admin.edit_hook(&hook);
                    admin.change_hooks(|hooks| hooks.push(hook.clone()));
                }
            },
        );
    }

    fn pick_hook_avatar(&mut self) {
        let parent = window_identifier();
        spawn_blocking(
            move || pick_icon(parent),
            |chat: &mut Chat, picked: Result<Option<ImageUpload>, String>| match picked {
                Ok(Some(image)) => chat.save_hook(Some(image)),
                Ok(None) => {}
                Err(error_message) => chat.manage_failed(error_message),
            },
        );
    }

    fn save_hook(&mut self, avatar: Option<ImageUpload>) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let admin = self.space_admin();
        let Some(hook) = admin.hook.as_ref().cloned() else {
            return;
        };
        let Some(channel): Option<ChannelId> = hook.channel else {
            return;
        };
        let name = admin.docs.hook_name.text();
        let id = hook.id;
        let avatar = avatar.map(Some);
        let closes = avatar.is_none();
        self.act(
            &key,
            move |client| {
                let (name, avatar) = (name.clone(), avatar.clone());
                async move { client.edit_webhook(&id, &name, &channel, avatar).await }
            },
            move |chat: &mut Chat, saved: Webhook| {
                let Some(admin) = chat.admin_for(space) else {
                    return;
                };
                admin.change_hooks(|hooks| {
                    if let Some(known) = hooks.iter_mut().find(|known| known.id == saved.id) {
                        *known = saved.clone();
                    }
                });
                admin.hook.set((!closes).then_some(saved));
            },
        );
    }

    fn ask_delete_hook(&mut self, hook: &discord_core::WebhookId) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let hook = *hook;
        self.confirm(
            Prompt {
                title: tr!("discord.admin.webhook-delete").to_string(),
                body: tr!("discord.admin.webhook-delete-body").to_string(),
                accept: tr!("discord.manage.delete").to_string(),
            },
            move |chat: &mut Chat| {
                chat.act(
                    &key,
                    move |client| async move { client.delete_webhook(&hook).await },
                    move |chat: &mut Chat, ()| {
                        if let Some(admin) = chat.admin_for(space) {
                            admin.hook.set(None);
                            admin.change_hooks(|hooks| hooks.retain(|known| known.id != hook));
                        }
                    },
                );
            },
        );
    }

    fn ask_remove_integration(&mut self, integration: &discord_core::IntegrationId) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let integration = *integration;
        self.confirm(
            Prompt {
                title: tr!("discord.admin.integration-remove").to_string(),
                body: tr!("discord.admin.integration-remove-body").to_string(),
                accept: tr!("discord.admin.remove").to_string(),
            },
            move |chat: &mut Chat| {
                chat.act(
                    &key,
                    move |client| async move { client.remove_integration(&space, &integration).await },
                    |chat: &mut Chat, ()| chat.load_integrations(),
                );
            },
        );
    }
}
