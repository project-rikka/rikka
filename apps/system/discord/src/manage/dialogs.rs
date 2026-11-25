use discord_core::{
    COMMUNITY, ChannelKind, INVITE_USES, InviteAge, PRUNE_DAYS, PrivacyChange, SpaceId,
    SpacePrivacy,
};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{Dialog, Manage, moderation};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::manage::rgb_of;
use crate::space_admin::toggle;

const WIDTH: f32 = 440.0;
const LONGEST_NICKNAME: usize = 32;
const LONGEST_CHANNEL: usize = 100;

pub(super) fn view(chat: &Chat, shown: &Dialog, stage: &'static str) -> Vec<Element> {
    let manage = chat.manage();
    let busy = manage.is_busy();
    let Some(linked) = dialog_account(chat, shown) else {
        return Vec::new();
    };
    let shaped = match shown {
        Dialog::Leave { space, .. } => leave(linked, space),
        Dialog::Privacy {
            privacy: chosen, ..
        } => privacy(*chosen),
        Dialog::Nickname { .. } => nickname(manage, busy),
        Dialog::CreateChannel {
            space,
            kind,
            private,
            ..
        } => create_channel(linked, manage, space, *kind, *private, busy),
        Dialog::Invite {
            request, invite, ..
        } => invite_view(request, invite.as_ref(), busy),
        Dialog::Folder { color, .. } => folder(manage, *color, busy),
        Dialog::DeleteChannel { channel, .. } => {
            let name = linked
                .channel(channel)
                .map(|channel| channel.name.clone())
                .unwrap_or_default();
            Shaped::new(
                tr!("discord.manage.delete-channel-title"),
                text(tr!(
                    "discord.manage.delete-channel-body",
                    name = name.as_str()
                ))
                .label()
                .fill_width()
                .any(),
                tr!("discord.manage.delete"),
            )
        }
        Dialog::DeleteSpace { space, .. } => delete_space(linked, manage, space),
        Dialog::Prune {
            space,
            days,
            roles,
            count,
            ..
        } => prune(linked, space, *days, roles, *count),
        Dialog::Kick { space, user, .. } => moderation::kick(linked, manage, space, user),
        Dialog::Ban {
            space, user, purge, ..
        } => moderation::ban(linked, manage, space, user, *purge),
        Dialog::Timeout {
            space, user, span, ..
        } => moderation::timeout(linked, manage, space, user, *span),
        Dialog::Roles { space, user, .. } => moderation::roles(linked, space, user),
        Dialog::Transfer { space, user, .. } => moderation::transfer(linked, space, user),
    };
    let ready = shaped.ready && !busy;
    let body = column! {
        shaped.body;
        if let Some(error_message) = manage.error_message() {
            text(error_message.to_owned()).caption().danger().fill_width();
        }
    }
    .gap(Space::NORMAL)
    .fill_width();
    let built = dialog(shaped.title, body).cancel(
        shaped
            .cancel
            .unwrap_or_else(|| tr!("discord.manage.cancel")),
        |manage: &mut Manage| manage.close(),
    );
    let built = match shaped.accept {
        Some(accept) => built
            .accept(accept, |chat: &mut Chat| chat.submit_dialog())
            .ready(ready),
        None => built,
    };
    built
        .width(WIDTH)
        .on_dismiss(|manage: &mut Manage| manage.close())
        .keyed("discord-manage-dialog")
        .centered_on(stage)
        .with_scrim()
}

fn dialog_account<'a>(chat: &'a Chat, dialog: &Dialog) -> Option<&'a LinkedAccount> {
    let key = match dialog {
        Dialog::Leave { key, .. }
        | Dialog::Privacy { key, .. }
        | Dialog::Nickname { key, .. }
        | Dialog::CreateChannel { key, .. }
        | Dialog::Invite { key, .. }
        | Dialog::Folder { key, .. }
        | Dialog::DeleteChannel { key, .. }
        | Dialog::Kick { key, .. }
        | Dialog::Ban { key, .. }
        | Dialog::Timeout { key, .. }
        | Dialog::Roles { key, .. }
        | Dialog::Transfer { key, .. }
        | Dialog::DeleteSpace { key, .. }
        | Dialog::Prune { key, .. } => key,
    };
    chat.linked(key)
}

pub(super) struct Shaped {
    pub(super) title: Label,
    pub(super) body: Element,
    pub(super) accept: Option<Label>,
    pub(super) cancel: Option<Label>,
    pub(super) ready: bool,
}

impl Shaped {
    pub(super) fn new(title: Label, body: Element, accept: Label) -> Shaped {
        Shaped {
            title,
            body,
            accept: Some(accept),
            cancel: None,
            ready: true,
        }
    }

    pub(super) fn ready(mut self, ready: bool) -> Shaped {
        self.ready = ready;
        self
    }

    pub(super) fn closing(mut self) -> Shaped {
        self.accept = None;
        self.cancel = Some(tr!("discord.manage.done"));
        self
    }
}

fn space_name(linked: &LinkedAccount, space: &SpaceId) -> String {
    linked
        .space(space)
        .map(|space| space.name.clone())
        .unwrap_or_default()
}

fn leave(linked: &LinkedAccount, space: &SpaceId) -> Shaped {
    let name = space_name(linked, space);
    Shaped::new(
        tr!("discord.manage.leave-title", name = name.as_str()),
        text(tr!("discord.manage.leave-body", name = name.as_str()))
            .label()
            .fill_width()
            .any(),
        tr!("discord.menu.leave"),
    )
}

fn privacy_toggle(
    label: Label,
    hint: Label,
    on: bool,
    change: fn(bool) -> PrivacyChange,
) -> Element {
    row! {
        column! {
            text(label).label().semibold();
            text(hint).caption().muted().fill_width();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        switch(on).on_toggle(move |chat: &mut Chat, on: bool| chat.change_privacy(change(on)));
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn privacy(privacy: SpacePrivacy) -> Shaped {
    Shaped::new(
        tr!("discord.manage.privacy-title"),
        column! {
            privacy_toggle(
                tr!("discord.manage.privacy-directs"),
                tr!("discord.manage.privacy-directs-hint"),
                privacy.directs,
                PrivacyChange::Directs,
            );
            privacy_toggle(
                tr!("discord.manage.privacy-requests"),
                tr!("discord.manage.privacy-requests-hint"),
                privacy.message_requests,
                PrivacyChange::MessageRequests,
            );
            privacy_toggle(
                tr!("discord.manage.privacy-activity"),
                tr!("discord.manage.privacy-activity-hint"),
                privacy.activity,
                PrivacyChange::Activity,
            );
            privacy_toggle(
                tr!("discord.manage.privacy-joining"),
                tr!("discord.manage.privacy-joining-hint"),
                privacy.joining,
                PrivacyChange::Joining,
            );
        }
        .gap(Space::NORMAL)
        .fill_width()
        .any(),
        tr!("discord.manage.done"),
    )
    .closing()
}

fn nickname(manage: &Manage, busy: bool) -> Shaped {
    Shaped::new(
        tr!("discord.manage.nickname-title"),
        column! {
            text(tr!("discord.manage.nickname")).caption().semibold().muted();
            text_field(manage.text_field())
                .max_len(LONGEST_NICKNAME)
                .autofocus(true)
                .enabled(!busy)
                .on_change(|manage: &mut Manage| manage.retext())
                .on_submit(|chat: &mut Chat| chat.submit_dialog())
                .fill_width();
            button(text(tr!("discord.manage.nickname-reset")))
                .ghost()
                .small()
                .enabled(!busy)
                .on_press(|chat: &mut Chat| chat.reset_nickname());
        }
        .gap(Space::TIGHT)
        .fill_width()
        .any(),
        tr!("discord.manage.save"),
    )
}

fn kind_label(kind: ChannelKind) -> Label {
    match kind {
        ChannelKind::Text => tr!("discord.manage.kind-text"),
        ChannelKind::Voice => tr!("discord.manage.kind-voice"),
        ChannelKind::Forum => tr!("discord.manage.kind-forum"),
        ChannelKind::Announcement => tr!("discord.manage.kind-announcement"),
        ChannelKind::Stage => tr!("discord.manage.kind-stage"),
        ChannelKind::Category => tr!("discord.manage.kind-category"),
        ChannelKind::Thread | ChannelKind::Direct | ChannelKind::Group => {
            tr!("discord.manage.kind-text")
        }
    }
}

fn create_channel(
    linked: &LinkedAccount,
    manage: &Manage,
    space: &SpaceId,
    kind: ChannelKind,
    private: bool,
    busy: bool,
) -> Shaped {
    let community = linked
        .space(space)
        .is_some_and(|space| space.has(COMMUNITY));
    let kinds: Vec<ChannelKind> = [
        ChannelKind::Text,
        ChannelKind::Voice,
        ChannelKind::Forum,
        ChannelKind::Announcement,
        ChannelKind::Stage,
    ]
    .into_iter()
    .filter(|kind| community || !matches!(kind, ChannelKind::Announcement | ChannelKind::Stage))
    .collect();
    let category = kind == ChannelKind::Category;
    let chosen = kinds.iter().position(|known| *known == kind).unwrap_or(0);
    let title = match category {
        true => tr!("discord.manage.create-category"),
        false => tr!("discord.manage.create-channel"),
    };
    Shaped::new(
        title,
        column! {
            if !category {
                text(tr!("discord.manage.channel-type")).caption().semibold().muted();
                kinds
                    .iter()
                    .fold(segmented(chosen), |choice, kind| choice.segment(kind_label(*kind)))
                    .on_select(move |manage: &mut Manage, at: usize| {
                        let picked = kinds.get(at).copied();
                        manage.change(|dialog| {
                            if let (Dialog::CreateChannel { kind, .. }, Some(picked)) = (dialog, picked) {
                                *kind = picked;
                            }
                        });
                    })
                    .fill_width();
            }
            text(tr!("discord.manage.channel-name")).caption().semibold().muted();
            text_field(manage.text_field())
                .max_len(LONGEST_CHANNEL)
                .autofocus(true)
                .enabled(!busy)
                .on_change(|manage: &mut Manage| manage.retext())
                .on_submit(|chat: &mut Chat| chat.submit_dialog())
                .fill_width();
            field(
                match category {
                    true => tr!("discord.manage.private-category"),
                    false => tr!("discord.manage.private-channel"),
                },
                switch(private).on_toggle(|manage: &mut Manage, on: bool| {
                    manage.change(|dialog| {
                        if let Dialog::CreateChannel { private, .. } = dialog {
                            *private = on;
                        }
                    });
                }),
            );
            text(tr!("discord.manage.private-hint")).caption().muted().fill_width();
        }
        .gap(Space::SNUG)
        .fill_width()
        .any(),
        tr!("discord.manage.create"),
    )
    .ready(manage.is_filled())
}

fn age_label(age: InviteAge) -> Label {
    match age {
        InviteAge::Minutes30 => tr!("discord.manage.age-30m"),
        InviteAge::Hour => tr!("discord.manage.age-1h"),
        InviteAge::Hours6 => tr!("discord.manage.age-6h"),
        InviteAge::Hours12 => tr!("discord.manage.age-12h"),
        InviteAge::Day => tr!("discord.manage.age-1d"),
        InviteAge::Week => tr!("discord.manage.age-7d"),
        InviteAge::Never => tr!("discord.manage.age-never"),
    }
}

fn uses_label(uses: u32) -> Label {
    match uses {
        0 => tr!("discord.manage.uses-unlimited"),
        uses => tr!("discord.manage.uses", count = i64::from(uses)),
    }
}

fn invite_view(
    request: &discord_core::InviteRequest,
    invite: Option<&discord_core::SpaceInvite>,
    busy: bool,
) -> Shaped {
    let link = invite.map(discord_core::SpaceInvite::url);
    let copied = link.clone().unwrap_or_default();
    let age_at = InviteAge::ALL
        .iter()
        .position(|age| *age == request.age)
        .unwrap_or(0);
    let uses_at = INVITE_USES
        .iter()
        .position(|uses| *uses == request.max_uses)
        .unwrap_or(0);
    Shaped::new(
        tr!("discord.manage.invite-title"),
        column! {
            text(tr!("discord.manage.invite-hint")).caption().muted().fill_width();
            row! {
                match &link {
                    Some(link) => text(link.clone()).label().semibold().grow(1.0).min_width(0.0).any(),
                    None => spinner().any(),
                };
                button(text(tr!("discord.manage.copy")))
                    .primary()
                    .small()
                    .enabled(link.is_some())
                    .on_press(move || {
                        set_text_of(Selection::Clipboard, &copied);
                        skip_view();
                    });
            }
            .gap(Space::SNUG)
            .align_items(Align::Center)
            .fill_width();
            field(
                tr!("discord.manage.invite-expires"),
                combo(age_at)
                    .options(InviteAge::ALL.map(age_label))
                    .enabled(!busy)
                    .on_select(|manage: &mut Manage, at: usize| {
                        manage.change_invite(|request| {
                            request.age = InviteAge::ALL[at.min(InviteAge::ALL.len() - 1)];
                        });
                    }),
            );
            field(
                tr!("discord.manage.invite-uses"),
                combo(uses_at)
                    .options(INVITE_USES.map(uses_label))
                    .enabled(!busy)
                    .on_select(|manage: &mut Manage, at: usize| {
                        manage.change_invite(|request| {
                            request.max_uses = INVITE_USES[at.min(INVITE_USES.len() - 1)];
                        });
                    }),
            );
            field(
                tr!("discord.manage.invite-temporary"),
                switch(request.temporary).enabled(!busy).on_toggle(|manage: &mut Manage, on: bool| {
                    manage.change_invite(|request| request.temporary = on);
                }),
            );
        }
        .gap(Space::SNUG)
        .fill_width()
        .any(),
        tr!("discord.manage.invite-generate"),
    )
}

fn folder(manage: &Manage, color: Option<u32>, busy: bool) -> Shaped {
    let shown = color.map_or_else(|| palette().accent, Color::hex);
    Shaped::new(
        tr!("discord.manage.folder-title"),
        column! {
            text(tr!("discord.manage.folder-name")).caption().semibold().muted();
            text_field(manage.text_field())
                .autofocus(true)
                .enabled(!busy)
                .on_change(|manage: &mut Manage| manage.retext())
                .on_submit(|chat: &mut Chat| chat.submit_dialog())
                .fill_width();
            text(tr!("discord.manage.folder-color")).caption().semibold().muted();
            row! {
                color_well(shown).on_commit(|manage: &mut Manage, picked: Color| {
                    manage.change(|dialog| {
                        if let Dialog::Folder { color, .. } = dialog {
                            *color = Some(rgb_of(picked));
                        }
                    });
                });
                button(text(tr!("discord.manage.color-default")))
                    .ghost()
                    .small()
                    .enabled(color.is_some())
                    .on_press(|manage: &mut Manage| {
                        manage.change(|dialog| {
                            if let Dialog::Folder { color, .. } = dialog {
                                *color = None;
                            }
                        });
                    });
            }
            .gap(Space::SNUG)
            .align_items(Align::Center);
        }
        .gap(Space::SNUG)
        .fill_width()
        .any(),
        tr!("discord.manage.save"),
    )
}

fn delete_space(linked: &LinkedAccount, manage: &Manage, space: &SpaceId) -> Shaped {
    let name = space_name(linked, space);
    let matches = manage.text() == name;
    Shaped::new(
        tr!("discord.manage.delete-space-title", name = name.as_str()),
        column! {
            text(tr!("discord.manage.delete-space-body")).label().fill_width();
            text(tr!("discord.manage.delete-space-confirm")).caption().semibold().muted();
            text_field(manage.text_field())
                .autofocus(true)
                .placeholder(name.clone())
                .on_change(|manage: &mut Manage| manage.retext())
                .fill_width();
        }
        .gap(Space::SNUG)
        .fill_width()
        .any(),
        tr!("discord.manage.delete-space"),
    )
    .ready(matches)
}

fn prune(
    linked: &LinkedAccount,
    space: &SpaceId,
    days: u8,
    included: &[discord_core::RoleId],
    count: Option<u32>,
) -> Shaped {
    let at = PRUNE_DAYS
        .iter()
        .position(|known| *known == days)
        .unwrap_or(0);
    let mut roles: Vec<&discord_core::Role> = linked
        .roles(space)
        .iter()
        .filter(|role| !role.is_everyone(space) && !role.managed)
        .collect();
    roles.sort_by_key(|role| std::cmp::Reverse(role.position));
    let summary = match count {
        Some(count) => tr!("discord.manage.prune-count", count = i64::from(count)),
        None => tr!("discord.manage.prune-counting"),
    };
    Shaped::new(
        tr!("discord.manage.prune-title"),
        column! {
            text(tr!("discord.manage.prune-hint")).caption().muted().fill_width();
            field(
                tr!("discord.manage.prune-days"),
                PRUNE_DAYS
                    .iter()
                    .fold(segmented(at), |choice, days| {
                        choice.segment(tr!("discord.manage.days", count = i64::from(*days)))
                    })
                    .on_select(|chat: &mut Chat, at: usize| {
                        chat.change_prune(|days, _| *days = PRUNE_DAYS[at.min(PRUNE_DAYS.len() - 1)]);
                    }),
            );
            if !roles.is_empty() {
                text(tr!("discord.manage.prune-roles")).caption().semibold().muted();
                column_of(roles.iter().map(|role| {
                    let role_id = role.id;
                    checkbox(included.contains(&role.id))
                        .label(text(role.name.clone()).label())
                        .on_toggle(move |chat: &mut Chat, on: bool| {
                            chat.change_prune(|_, roles| toggle(roles, role_id, on));
                        })
                        .any()
                }))
                .gap(Space::HAIR)
                .fill_width();
            }
            text(summary).label().semibold().fill_width();
        }
        .gap(Space::SNUG)
        .fill_width()
        .any(),
        tr!("discord.manage.prune"),
    )
    .ready(count.is_some_and(|count| count > 0))
}
