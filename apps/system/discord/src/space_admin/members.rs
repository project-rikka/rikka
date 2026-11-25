use discord_core::{MemberHit, Permissions, SpaceId};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{SpaceAdmin, loading};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::conversation::time::day_label;
use crate::widgets::{avatar, mark_of};

const ROLES_SHOWN: usize = 3;

pub(super) fn view(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let (key, space) = (linked.key().to_owned(), *space);
    let prunes = linked
        .space(&space)
        .is_some_and(|known| known.can(Permissions::KICK_MEMBERS));
    let list = loading(&admin.members, |page| {
        column! {
            text(tr!("discord.admin.member-count", count = i64::from(page.total))).caption().muted();
            column_of(page.hits.iter().map(|hit| member_row(linked, &space, hit)))
                .gap(Space::TIGHT)
                .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width()
        .any()
    });
    column! {
        row! {
            text_field(&admin.docs.search)
                .placeholder(tr!("discord.admin.search-members"))
                .on_submit(|chat: &mut Chat| chat.search_admin_members())
                .grow(1.0);
            button(text(tr!("discord.admin.search")))
                .secondary()
                .small()
                .on_press(|chat: &mut Chat| chat.search_admin_members());
            if prunes {
                button(text(tr!("discord.manage.prune")))
                    .ghost()
                    .small()
                    .on_press(move |chat: &mut Chat| chat.ask_prune(&key, &space));
            }
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        list;
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

fn member_row(linked: &LinkedAccount, space: &SpaceId, hit: &MemberHit) -> Element {
    let name = hit
        .member
        .nickname
        .clone()
        .unwrap_or_else(|| hit.user.name().to_owned());
    let roles: Vec<String> = hit
        .member
        .roles
        .iter()
        .filter_map(|role| linked.role(space, role))
        .take(ROLES_SHOWN)
        .map(|role| role.name.clone())
        .collect();
    let extra = hit.member.roles.len().saturating_sub(ROLES_SHOWN);
    let joined = hit.member.joined.map(day_label).unwrap_or_default();
    let mark = mark_of(("discord-admin-member", hit.user.id));
    let (user, space) = (hit.user.id, *space);
    card(
        row! {
            avatar(hit.member.avatar.as_ref().or(hit.user.avatar.as_ref()), &name, 36.0);
            column! {
                text(name.clone()).label().semibold().no_wrap();
                text(hit.user.handle.clone()).caption().muted().no_wrap();
            }
            .gap(Space::HAIR)
            .width(180.0);
            column! {
                text(joined).caption().muted().no_wrap();
                if let Some(invite) = &hit.invite {
                    text(format!("discord.gg/{invite}")).caption().muted().no_wrap();
                }
            }
            .gap(Space::HAIR)
            .width(140.0);
            text(match extra {
                0 => roles.join(", "),
                extra => format!("{} +{extra}", roles.join(", ")),
            })
            .caption()
            .no_wrap()
            .grow(1.0)
            .min_width(0.0);
            button(icon(MaterialSymbol::MORE_HORIZ).extent(Length::px(16.0)))
                .ghost()
                .small()
                .on_press(move |chat: &mut Chat| {
                    chat.open_member_menu(&user, Some(&space), Anchor::new(mark, AnchorEdge::BelowEnd));
                })
                .build()
                .keyed(mark);
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .any()
}

impl Chat {
    pub(super) fn search_admin_members(&mut self) {
        let query = self.space_admin().docs.search.text().trim().to_owned();
        self.space_admin_mut().member_query = query.clone();
        let asked = query.clone();
        self.fetch(
            move |client, space| async move { client.search_members(&space, &query).await },
            move |admin, page| {
                if admin.member_query == asked {
                    admin.members.set(Some(page));
                }
            },
        );
    }
}
