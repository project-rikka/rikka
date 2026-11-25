use discord_core::{PurgeSpan, Role, SpaceId, TimeoutSpan, UserId};
use rikka_app::prelude::*;
use rikka_widgets::column;

use super::dialogs::Shaped;
use super::{Dialog, Manage};
use crate::account::LinkedAccount;
use crate::chat::Chat;

const LONGEST_REASON: usize = 512;
const ROLES_MOST: f32 = 360.0;

fn name(linked: &LinkedAccount, space: &SpaceId, user: &UserId) -> String {
    linked
        .name_of(Some(space), user)
        .unwrap_or_else(|| user.to_string())
}

fn reason(manage: &Manage) -> Element {
    column! {
        text(tr!("discord.moderation.reason")).caption().semibold().muted();
        text_field(manage.reason_field())
            .max_len(LONGEST_REASON)
            .enabled(!manage.is_busy())
            .fill_width();
    }
    .gap(Space::TIGHT)
    .fill_width()
    .any()
}

pub(super) fn kick(
    linked: &LinkedAccount,
    manage: &Manage,
    space: &SpaceId,
    user: &UserId,
) -> Shaped {
    let name = name(linked, space, user);
    Shaped::new(
        tr!("discord.moderation.kick-title", name = name.as_str()),
        column! {
            text(tr!("discord.moderation.kick-body", name = name.as_str())).label().fill_width();
            reason(manage);
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any(),
        tr!("discord.moderation.kick"),
    )
}

fn purge_label(purge: PurgeSpan) -> Label {
    match purge {
        PurgeSpan::None => tr!("discord.moderation.purge-none"),
        PurgeSpan::Hour => tr!("discord.moderation.purge-hour"),
        PurgeSpan::Hours6 => tr!("discord.moderation.purge-6h"),
        PurgeSpan::Hours12 => tr!("discord.moderation.purge-12h"),
        PurgeSpan::Day => tr!("discord.moderation.purge-day"),
        PurgeSpan::Days3 => tr!("discord.moderation.purge-3d"),
        PurgeSpan::Days7 => tr!("discord.moderation.purge-7d"),
    }
}

pub(super) fn ban(
    linked: &LinkedAccount,
    manage: &Manage,
    space: &SpaceId,
    user: &UserId,
    purge: PurgeSpan,
) -> Shaped {
    let name = name(linked, space, user);
    let at = PurgeSpan::ALL
        .iter()
        .position(|known| *known == purge)
        .unwrap_or(0);
    Shaped::new(
        tr!("discord.moderation.ban-title", name = name.as_str()),
        column! {
            text(tr!("discord.moderation.ban-body", name = name.as_str())).label().fill_width();
            field(
                tr!("discord.moderation.purge"),
                combo(at)
                    .options(PurgeSpan::ALL.map(purge_label))
                    .on_select(|manage: &mut Manage, at: usize| {
                        manage.change(|dialog| {
                            if let Dialog::Ban { purge, .. } = dialog {
                                *purge = PurgeSpan::ALL[at.min(PurgeSpan::ALL.len() - 1)];
                            }
                        });
                    }),
            );
            reason(manage);
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any(),
        tr!("discord.moderation.ban"),
    )
}

pub(crate) fn timeout_label(span: TimeoutSpan) -> Label {
    match span {
        TimeoutSpan::Minute => tr!("discord.moderation.timeout-1m"),
        TimeoutSpan::Minutes5 => tr!("discord.moderation.timeout-5m"),
        TimeoutSpan::Minutes10 => tr!("discord.moderation.timeout-10m"),
        TimeoutSpan::Hour => tr!("discord.moderation.timeout-1h"),
        TimeoutSpan::Day => tr!("discord.moderation.timeout-1d"),
        TimeoutSpan::Week => tr!("discord.moderation.timeout-1w"),
    }
}

pub(super) fn timeout(
    linked: &LinkedAccount,
    manage: &Manage,
    space: &SpaceId,
    user: &UserId,
    span: TimeoutSpan,
) -> Shaped {
    let name = name(linked, space, user);
    let at = TimeoutSpan::ALL
        .iter()
        .position(|known| *known == span)
        .unwrap_or(0);
    Shaped::new(
        tr!("discord.moderation.timeout-title", name = name.as_str()),
        column! {
            text(tr!("discord.moderation.timeout-body")).label().fill_width();
            TimeoutSpan::ALL
                .iter()
                .fold(segmented(at), |choice, span| choice.segment(timeout_label(*span)))
                .on_select(|manage: &mut Manage, at: usize| {
                    manage.change(|dialog| {
                        if let Dialog::Timeout { span, .. } = dialog {
                            *span = TimeoutSpan::ALL[at.min(TimeoutSpan::ALL.len() - 1)];
                        }
                    });
                })
                .fill_width();
            reason(manage);
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any(),
        tr!("discord.moderation.timeout"),
    )
}

fn assignable<'a>(linked: &'a LinkedAccount, space: &SpaceId) -> Vec<&'a Role> {
    let Some(me) = linked.me().map(|me| me.id) else {
        return Vec::new();
    };
    let owner = linked.space(space).is_some_and(|known| known.owned_by(&me));
    let highest = linked.top_role_position(space, &me);
    let mut roles: Vec<&Role> = linked
        .roles(space)
        .iter()
        .filter(|role| !role.is_everyone(space) && !role.managed)
        .filter(|role| owner || role.position < highest)
        .collect();
    roles.sort_by_key(|role| std::cmp::Reverse(role.position));
    roles
}

pub(super) fn roles(linked: &LinkedAccount, space: &SpaceId, user: &UserId) -> Shaped {
    let held = linked
        .member(space, user)
        .map(|member| member.roles.clone())
        .unwrap_or_default();
    let name = name(linked, space, user);
    let rows = assignable(linked, space).into_iter().map(|role| {
        let role_id = role.id;
        let tint = role.color.map_or_else(|| palette().text_muted, Color::hex);
        checkbox(held.contains(&role.id))
            .label(text(role.name.clone()).label().color(tint))
            .on_toggle(move |chat: &mut Chat, on: bool| chat.toggle_member_role(&role_id, on))
            .any()
    });
    Shaped::new(
        tr!("discord.moderation.roles-title", name = name.as_str()),
        scrollable(column_of(rows).gap(Space::HAIR).fill_width())
            .max_height(ROLES_MOST)
            .fill_width()
            .any(),
        tr!("discord.manage.done"),
    )
    .closing()
}

pub(super) fn transfer(linked: &LinkedAccount, space: &SpaceId, user: &UserId) -> Shaped {
    let name = name(linked, space, user);
    Shaped::new(
        tr!("discord.moderation.transfer-title"),
        text(tr!(
            "discord.moderation.transfer-body",
            name = name.as_str()
        ))
        .label()
        .fill_width()
        .any(),
        tr!("discord.moderation.transfer"),
    )
}
