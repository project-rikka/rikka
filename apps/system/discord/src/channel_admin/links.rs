use discord_core::{ChannelId, Webhook};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{ChannelAdmin, Opened};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::space_admin::{hint, invite_row, loading};
use crate::widgets::avatar;

pub(super) fn invites(admin: &ChannelAdmin, opened: &Opened) -> Element {
    loading(&admin.invites, |invites| {
        let invited = opened.clone();
        column! {
            button(text(tr!("discord.menu.invite")))
                .primary()
                .small()
                .on_press(move |chat: &mut Chat| chat.open_invite(&invited.key, &invited.channel));
            column_of(invites.iter().map(|invite| invite_row(&opened.key, invite))).gap(Space::TIGHT).fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any()
    })
}

pub(super) fn hooks(admin: &ChannelAdmin) -> Element {
    loading(&admin.hooks, |hooks| {
        column! {
            hint(tr!("discord.admin.webhooks-hint"));
            button(text(tr!("discord.admin.new-webhook")))
                .primary()
                .small()
                .on_press(|chat: &mut Chat| chat.create_channel_hook());
            column_of(hooks.iter().map(hook_row)).gap(Space::TIGHT).fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any()
    })
}

fn hook_row(hook: &Webhook) -> Element {
    let url = hook.url.clone().unwrap_or_default();
    let id = hook.id;
    card(
        row! {
            avatar(hook.avatar.as_ref(), &hook.name, 32.0);
            text(hook.name.clone()).label().semibold().grow(1.0).min_width(0.0);
            button(text(tr!("discord.admin.webhook-copy-url")))
                .secondary()
                .small()
                .enabled(!url.is_empty())
                .on_press(move || {
                    set_text_of(Selection::Clipboard, &url);
                    skip_view();
                });
            button(icon(MaterialSymbol::DELETE).extent(Length::px(16.0)))
                .ghost()
                .small()
                .on_press(move |chat: &mut Chat| chat.delete_channel_hook(&id));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .any()
}

pub(super) fn delete(linked: &LinkedAccount, channel: &ChannelId) -> Element {
    let (key, channel) = (linked.key().to_owned(), *channel);
    column! {
        hint(tr!("discord.channel-admin.delete-hint"));
        button(text(tr!("discord.channel-admin.delete")))
            .danger()
            .on_press(move |chat: &mut Chat| chat.ask_delete_channel(&key, &channel));
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

impl Chat {
    fn create_channel_hook(&mut self) {
        let Some((key, channel)) = self.channel_target() else {
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
                if let Some(admin) = chat.channel_admin_for(channel) {
                    admin.change_hooks(|hooks| hooks.push(hook.clone()));
                }
            },
        );
    }

    fn delete_channel_hook(&mut self, hook: &discord_core::WebhookId) {
        let Some((key, channel)) = self.channel_target() else {
            return;
        };
        let hook = *hook;
        self.act(
            &key,
            move |client| async move { client.delete_webhook(&hook).await },
            move |chat: &mut Chat, ()| {
                if let Some(admin) = chat.channel_admin_for(channel) {
                    admin.change_hooks(|hooks| hooks.retain(|known| known.id != hook));
                }
            },
        );
    }
}

impl ChannelAdmin {
    fn change_hooks(&mut self, change: impl FnOnce(&mut Vec<Webhook>)) {
        if let Some(Ok(mut hooks)) = (*self.hooks).clone() {
            change(&mut hooks);
            self.hooks.set(Some(Ok(hooks)));
        }
    }
}
