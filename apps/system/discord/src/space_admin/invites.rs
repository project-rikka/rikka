use discord_core::{SpaceId, SpaceInvite};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{Section, SpaceAdmin, hint, loading};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::conversation::time::sent_at;
use crate::widgets::avatar;

pub(super) fn view(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let (key, space) = (linked.key().to_owned(), *space);
    loading(&admin.invites, |invites| {
        column! {
            hint(tr!("discord.admin.invites-hint"));
            button(text(tr!("discord.menu.invite")))
                .primary()
                .small()
                .on_press({
                    let key = key.clone();
                    move |chat: &mut Chat| chat.invite_to_space(&key, &space)
                });
            column_of(invites.iter().map(|invite| invite_row(&key, invite))).gap(Space::TIGHT).fill_width();
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any()
    })
}

pub(crate) fn invite_row(key: &str, invite: &SpaceInvite) -> Element {
    let (key, code) = (key.to_owned(), invite.code.clone());
    let inviter = invite
        .inviter
        .as_ref()
        .map_or_else(String::new, |user| user.name().to_owned());
    let expires = match invite.expires {
        Some(expires) => tr!(
            "discord.admin.invite-expires",
            time = sent_at(expires).as_str()
        ),
        None => tr!("discord.manage.age-never"),
    };
    let uses = match invite.max_uses {
        0 => tr!("discord.admin.invite-uses", count = i64::from(invite.uses)),
        limit => tr!(
            "discord.admin.invite-uses-of",
            count = i64::from(invite.uses),
            limit = i64::from(limit)
        ),
    };
    card(
        row! {
            avatar(invite.inviter.as_ref().and_then(|user| user.avatar.as_ref()), &inviter, 28.0);
            column! {
                text(inviter.clone()).label().semibold().no_wrap();
                text(invite.code.clone()).caption().muted().no_wrap();
            }
            .gap(Space::HAIR)
            .width(180.0);
            text(invite.channel_name.clone().map(|name| format!("# {name}")).unwrap_or_default())
                .caption()
                .no_wrap()
                .width(140.0);
            text(uses).caption().no_wrap().width(120.0);
            text(expires).caption().muted().no_wrap().grow(1.0).min_width(0.0);
            if invite.temporary {
                text(tr!("discord.manage.invite-temporary")).caption().muted();
            }
            button(text(tr!("discord.admin.revoke")))
                .danger()
                .small()
                .on_press(move |chat: &mut Chat| chat.revoke_invite(&key, &code));
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width(),
    )
    .any()
}

impl Chat {
    pub(super) fn load_invites(&mut self) {
        self.fetch(
            |client, space| async move { client.space_invites(&space).await },
            |admin, invites| admin.invites.set(Some(invites)),
        );
    }

    pub(crate) fn refresh_space_invites(&mut self) {
        if self.space_admin().is_open() && self.space_admin().section() == Section::Invites {
            self.load_invites();
        }
    }

    fn revoke_invite(&mut self, key: &str, code: &str) {
        let code = code.to_owned();
        self.act(
            key,
            move |client| {
                let code = code.clone();
                async move { client.revoke_invite(&code).await }
            },
            |chat: &mut Chat, ()| {
                chat.refresh_space_invites();
                chat.refresh_channel_invites();
            },
        );
    }
}
