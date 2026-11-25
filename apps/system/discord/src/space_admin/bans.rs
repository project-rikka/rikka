use discord_core::{Ban, UserId};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{SpaceAdmin, hint, loading};
use crate::chat::Chat;
use crate::manage::Prompt;
use crate::widgets::avatar;

pub(super) fn view(admin: &SpaceAdmin) -> Element {
    let filter = admin.ban_filter.to_lowercase();
    loading(&admin.bans, |bans| {
        let shown: Vec<&Ban> = bans
            .iter()
            .filter(|ban| {
                filter.is_empty()
                    || ban.user.handle.to_lowercase().contains(&filter)
                    || ban.user.name().to_lowercase().contains(&filter)
                    || ban.user.id.to_string() == filter
            })
            .collect();
        column! {
            hint(tr!("discord.admin.bans-hint", count = bans.len() as i64));
            text_field(&admin.docs.ban_filter)
                .placeholder(tr!("discord.admin.search-bans"))
                .on_change(|admin: &mut SpaceAdmin| {
                    let filter = admin.docs.ban_filter.text().trim().to_owned();
                    admin.ban_filter.set(filter);
                })
                .fill_width();
            column_of(shown.into_iter().map(ban_row)).gap(Space::TIGHT).fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any()
    })
}

fn ban_row(ban: &Ban) -> Element {
    let user = ban.user.id;
    let name = ban.user.name().to_owned();
    card(
        row! {
            avatar(ban.user.avatar.as_ref(), &name, 32.0);
            column! {
                text(name.clone()).label().semibold().no_wrap();
                text(ban.reason.clone().unwrap_or_else(|| tr!("discord.admin.no-reason").to_string()))
                    .caption()
                    .muted()
                    .fill_width();
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            button(text(tr!("discord.admin.revoke-ban")))
                .secondary()
                .small()
                .on_press(move |chat: &mut Chat| chat.ask_unban(&user, &name));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .any()
}

impl Chat {
    pub(super) fn load_bans(&mut self) {
        self.fetch(
            |client, space| async move { client.bans(&space).await },
            |admin, bans| admin.bans.set(Some(bans)),
        );
    }

    fn ask_unban(&mut self, user: &UserId, name: &str) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let user = *user;
        self.confirm(
            Prompt {
                title: tr!("discord.admin.revoke-ban-title", name = name).to_string(),
                body: tr!("discord.admin.revoke-ban-body").to_string(),
                accept: tr!("discord.admin.revoke-ban").to_string(),
            },
            move |chat: &mut Chat| {
                chat.act(
                    &key,
                    move |client| async move { client.unban(&space, &user).await },
                    |chat: &mut Chat, ()| chat.load_bans(),
                );
            },
        );
    }
}
