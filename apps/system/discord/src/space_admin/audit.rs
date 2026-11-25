use discord_core::{AuditAction, AuditEntry, AuditQuery, ChannelId, RoleId, SpaceId, User, UserId};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{SpaceAdmin, loading};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::conversation::time::sent_at;
use crate::widgets::avatar;

const FILTERS: [AuditAction; 16] = [
    AuditAction::GUILD_UPDATE,
    AuditAction::CHANNEL_CREATE,
    AuditAction::CHANNEL_UPDATE,
    AuditAction::CHANNEL_DELETE,
    AuditAction::MEMBER_KICK,
    AuditAction::MEMBER_BAN_ADD,
    AuditAction::MEMBER_BAN_REMOVE,
    AuditAction::MEMBER_UPDATE,
    AuditAction::MEMBER_ROLE_UPDATE,
    AuditAction::ROLE_CREATE,
    AuditAction::ROLE_UPDATE,
    AuditAction::ROLE_DELETE,
    AuditAction::INVITE_CREATE,
    AuditAction::WEBHOOK_CREATE,
    AuditAction::EMOJI_CREATE,
    AuditAction::MESSAGE_DELETE,
];

pub(crate) fn action_label(action: AuditAction) -> Label {
    match action {
        AuditAction::GUILD_UPDATE => tr!("discord.audit.guild-update"),
        AuditAction::CHANNEL_CREATE => tr!("discord.audit.channel-create"),
        AuditAction::CHANNEL_UPDATE => tr!("discord.audit.channel-update"),
        AuditAction::CHANNEL_DELETE => tr!("discord.audit.channel-delete"),
        AuditAction::CHANNEL_OVERWRITE_CREATE
        | AuditAction::CHANNEL_OVERWRITE_UPDATE
        | AuditAction::CHANNEL_OVERWRITE_DELETE => tr!("discord.audit.overwrite"),
        AuditAction::MEMBER_KICK => tr!("discord.audit.member-kick"),
        AuditAction::MEMBER_PRUNE => tr!("discord.audit.member-prune"),
        AuditAction::MEMBER_BAN_ADD => tr!("discord.audit.ban-add"),
        AuditAction::MEMBER_BAN_REMOVE => tr!("discord.audit.ban-remove"),
        AuditAction::MEMBER_UPDATE => tr!("discord.audit.member-update"),
        AuditAction::MEMBER_ROLE_UPDATE => tr!("discord.audit.member-roles"),
        AuditAction::MEMBER_MOVE => tr!("discord.audit.member-move"),
        AuditAction::MEMBER_DISCONNECT => tr!("discord.audit.member-disconnect"),
        AuditAction::BOT_ADD => tr!("discord.audit.bot-add"),
        AuditAction::ROLE_CREATE => tr!("discord.audit.role-create"),
        AuditAction::ROLE_UPDATE => tr!("discord.audit.role-update"),
        AuditAction::ROLE_DELETE => tr!("discord.audit.role-delete"),
        AuditAction::INVITE_CREATE => tr!("discord.audit.invite-create"),
        AuditAction::INVITE_UPDATE => tr!("discord.audit.invite-update"),
        AuditAction::INVITE_DELETE => tr!("discord.audit.invite-delete"),
        AuditAction::WEBHOOK_CREATE => tr!("discord.audit.webhook-create"),
        AuditAction::WEBHOOK_UPDATE => tr!("discord.audit.webhook-update"),
        AuditAction::WEBHOOK_DELETE => tr!("discord.audit.webhook-delete"),
        AuditAction::EMOJI_CREATE | AuditAction::STICKER_CREATE | AuditAction::SOUND_CREATE => {
            tr!("discord.audit.expression-create")
        }
        AuditAction::EMOJI_UPDATE | AuditAction::STICKER_UPDATE | AuditAction::SOUND_UPDATE => {
            tr!("discord.audit.expression-update")
        }
        AuditAction::EMOJI_DELETE | AuditAction::STICKER_DELETE | AuditAction::SOUND_DELETE => {
            tr!("discord.audit.expression-delete")
        }
        AuditAction::MESSAGE_DELETE | AuditAction::MESSAGE_BULK_DELETE => {
            tr!("discord.audit.message-delete")
        }
        AuditAction::MESSAGE_PIN => tr!("discord.audit.message-pin"),
        AuditAction::MESSAGE_UNPIN => tr!("discord.audit.message-unpin"),
        AuditAction::INTEGRATION_CREATE
        | AuditAction::INTEGRATION_UPDATE
        | AuditAction::INTEGRATION_DELETE => tr!("discord.audit.integration"),
        AuditAction::THREAD_CREATE | AuditAction::THREAD_UPDATE | AuditAction::THREAD_DELETE => {
            tr!("discord.audit.thread")
        }
        AuditAction::EVENT_CREATE | AuditAction::EVENT_UPDATE | AuditAction::EVENT_DELETE => {
            tr!("discord.audit.event")
        }
        AuditAction::AUTOMOD_RULE_CREATE
        | AuditAction::AUTOMOD_RULE_UPDATE
        | AuditAction::AUTOMOD_RULE_DELETE => tr!("discord.audit.automod-rule"),
        AuditAction::AUTOMOD_BLOCK_MESSAGE
        | AuditAction::AUTOMOD_FLAG_TO_CHANNEL
        | AuditAction::AUTOMOD_TIMEOUT => tr!("discord.audit.automod-action"),
        AuditAction::ONBOARDING_PROMPT_CREATE
        | AuditAction::ONBOARDING_PROMPT_UPDATE
        | AuditAction::ONBOARDING_PROMPT_DELETE
        | AuditAction::ONBOARDING_CREATE
        | AuditAction::ONBOARDING_UPDATE => tr!("discord.audit.onboarding"),
        other => tr!("discord.audit.other", code = i64::from(other.0)),
    }
}

fn target_name(linked: &LinkedAccount, space: &SpaceId, target: &str) -> String {
    let Ok(raw) = target.parse::<u64>() else {
        return target.to_owned();
    };
    if let Some(user) = linked.user(&UserId::new(raw)) {
        return linked.display_name(Some(space), user);
    }
    if let Some(channel) = linked.channel(&ChannelId::new(raw)) {
        return format!("# {}", channel.name);
    }
    if let Some(role) = linked.role(space, &RoleId::new(raw)) {
        return format!("@{}", role.name);
    }
    target.to_owned()
}

pub(super) fn view(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let query = (*admin.audit_query).clone();
    let action_at = query
        .action
        .and_then(|action| FILTERS.iter().position(|known| *known == action))
        .map_or(0, |at| at + 1);
    let mut options: Vec<Label> = vec![tr!("discord.audit.all-actions")];
    options.extend(FILTERS.map(action_label));
    let actors: Vec<User> = match admin.audit.as_ref() {
        Some(Ok(entries)) => {
            let mut actors: Vec<User> = Vec::new();
            for actor in entries.iter().filter_map(|entry| entry.actor.clone()) {
                if !actors.iter().any(|known| known.id == actor.id) {
                    actors.push(actor);
                }
            }
            actors
        }
        _ => Vec::new(),
    };
    let actor_at = query
        .actor
        .and_then(|actor| actors.iter().position(|known| known.id == actor))
        .map_or(0, |at| at + 1);
    let mut actor_options: Vec<Label> = vec![tr!("discord.audit.all-users")];
    actor_options.extend(
        actors
            .iter()
            .map(|user| Label::from(user.name().to_owned())),
    );
    let actor_ids: Vec<UserId> = actors.iter().map(|user| user.id).collect();
    let space = *space;
    let list = loading(&admin.audit, |entries| {
        column! {
            column_of(entries.iter().map(|entry| entry_row(linked, &space, entry)))
                .gap(Space::TIGHT)
                .fill_width();
            if !*admin.audit_end && !entries.is_empty() {
                button(text(tr!("discord.audit.more")))
                    .secondary()
                    .small()
                    .on_press(|chat: &mut Chat| chat.load_audit(false));
            }
        }
        .gap(Space::SNUG)
        .fill_width()
        .any()
    });
    column! {
        row! {
            combo(actor_at)
                .options(actor_options)
                .on_select(move |chat: &mut Chat, at: usize| {
                    let actor = at.checked_sub(1).and_then(|at| actor_ids.get(at)).copied();
                    chat.filter_audit(|query| query.actor = actor);
                })
                .width(220.0);
            combo(action_at)
                .options(options)
                .on_select(|chat: &mut Chat, at: usize| {
                    let action = at.checked_sub(1).and_then(|at| FILTERS.get(at)).copied();
                    chat.filter_audit(|query| query.action = action);
                })
                .width(260.0);
        }
        .gap(Space::SNUG);
        list;
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

fn entry_row(linked: &LinkedAccount, space: &SpaceId, entry: &AuditEntry) -> Element {
    let actor = entry.actor.as_ref().map_or_else(
        || tr!("discord.audit.someone").to_string(),
        |user| linked.display_name(Some(space), user),
    );
    let target = entry
        .target
        .as_deref()
        .map(|target| target_name(linked, space, target));
    card(
        column! {
            row! {
                avatar(entry.actor.as_ref().and_then(|user| user.avatar.as_ref()), &actor, 28.0);
                text(actor.clone()).label().semibold().no_wrap();
                text(action_label(entry.action)).label().no_wrap();
                if let Some(target) = target {
                    text(target).label().semibold().no_wrap().min_width(0.0);
                }
                spacer().grow(1.0);
                text(sent_at(entry.id.created_at())).caption().muted().no_wrap();
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            column_of(entry.changes.iter().map(|change| {
                let shown = match (&change.old, &change.new) {
                    (Some(old), Some(new)) => format!("{}: {old} → {new}", change.key),
                    (None, Some(new)) => format!("{}: {new}", change.key),
                    (Some(old), None) => format!("{}: {old} → —", change.key),
                    (None, None) => change.key.clone(),
                };
                text(shown).caption().muted().fill_width().any()
            }))
            .gap(Space::HAIR)
            .fill_width();
            if let Some(reason) = &entry.reason {
                text(tr!("discord.audit.reason", reason = reason.as_str())).caption().fill_width();
            }
        }
        .gap(Space::TIGHT)
        .fill_width(),
    )
    .any()
}

impl Chat {
    pub(super) fn load_audit(&mut self, fresh: bool) {
        let admin = self.space_admin();
        let mut query: AuditQuery = (*admin.audit_query).clone();
        query.before = match (fresh, admin.audit.as_ref()) {
            (false, Some(Ok(entries))) => entries.last().map(|entry| entry.id),
            _ => None,
        };
        if fresh {
            self.space_admin_mut().audit.set(None);
        }
        let asked = query.clone();
        self.fetch(
            move |client, space| async move { client.audit_log(&space, &query).await },
            move |admin, loaded| {
                let current = &*admin.audit_query;
                if current.actor != asked.actor || current.action != asked.action {
                    return;
                }
                let page = match loaded {
                    Ok(page) => page,
                    Err(error_message) => return admin.audit.set(Some(Err(error_message))),
                };
                let mut entries = match (asked.before, (*admin.audit).clone()) {
                    (None, _) => Vec::new(),
                    (Some(before), Some(Ok(entries)))
                        if entries.last().map(|entry| entry.id) == Some(before) =>
                    {
                        entries
                    }
                    (Some(_), _) => return,
                };
                admin
                    .audit_end
                    .set(page.len() < discord_core::AUDIT_PAGE as usize);
                entries.extend(page);
                admin.audit.set(Some(Ok(entries)));
            },
        );
    }

    fn filter_audit(&mut self, change: impl FnOnce(&mut AuditQuery)) {
        let mut query = (*self.space_admin().audit_query).clone();
        change(&mut query);
        query.before = None;
        self.space_admin_mut().audit_query.set(query);
        self.load_audit(true);
    }
}
