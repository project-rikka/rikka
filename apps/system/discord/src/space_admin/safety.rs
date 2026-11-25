use discord_core::{ContentFilter, SpaceId, SpaceSettings, VerificationLevel};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::overview::{channel_combo, channels};
use super::{SpaceAdmin, hint, loading};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::chat::now_ms;
use crate::conversation::time::sent_at;

const PAUSES: [i64; 5] = [1, 3, 6, 12, 24];
const HOUR_MS: i64 = 3_600_000;

fn verification_label(level: VerificationLevel) -> (Label, Label) {
    match level {
        VerificationLevel::None => (
            tr!("discord.admin.verification-none"),
            tr!("discord.admin.verification-none-hint"),
        ),
        VerificationLevel::Low => (
            tr!("discord.admin.verification-low"),
            tr!("discord.admin.verification-low-hint"),
        ),
        VerificationLevel::Medium => (
            tr!("discord.admin.verification-medium"),
            tr!("discord.admin.verification-medium-hint"),
        ),
        VerificationLevel::High => (
            tr!("discord.admin.verification-high"),
            tr!("discord.admin.verification-high-hint"),
        ),
        VerificationLevel::Highest => (
            tr!("discord.admin.verification-highest"),
            tr!("discord.admin.verification-highest-hint"),
        ),
    }
}

fn filter_label(filter: ContentFilter) -> Label {
    match filter {
        ContentFilter::Disabled => tr!("discord.admin.filter-disabled"),
        ContentFilter::MembersWithoutRoles => tr!("discord.admin.filter-no-roles"),
        ContentFilter::AllMembers => tr!("discord.admin.filter-all"),
    }
}

fn choice_row(
    selected: bool,
    label: Label,
    detail: Label,
    pick: impl Fn(&mut SpaceAdmin) + 'static,
) -> Element {
    row! {
        radio(selected).on_select(move |admin: &mut SpaceAdmin| pick(admin));
        column! {
            text(label).label().semibold();
            text(detail).caption().muted().fill_width();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

pub(super) fn view(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    let owner = linked
        .me()
        .zip(linked.space(space))
        .is_some_and(|(me, known)| known.owned_by(&me.id));
    loading(&admin.settings, |edited| {
        let settings = &edited.draft;
        column! {
            section(tr!("discord.admin.verification"), verification(settings));
            section(tr!("discord.admin.content-filter"), content_filter(settings));
            if owner {
                section(
                    tr!("discord.admin.mfa"),
                    column! {
                        field(
                            tr!("discord.admin.mfa-require"),
                            switch(settings.mfa_required).on_toggle(|admin: &mut SpaceAdmin, on: bool| {
                                admin.edit_settings(|settings| settings.mfa_required = on);
                            }),
                        );
                        hint(tr!("discord.admin.mfa-hint"));
                    }
                    .gap(Space::SNUG)
                    .fill_width(),
                );
            }
            if settings.is_community() {
                section(
                    tr!("discord.admin.safety-channel"),
                    channel_combo(
                        settings.safety_channel,
                        channels(linked, space, false),
                        Some(tr!("discord.admin.no-channel")),
                        |admin, chosen| admin.edit_settings(|settings| settings.safety_channel = chosen),
                    ),
                );
            }
            section(tr!("discord.admin.incidents"), incidents(settings));
        }
        .gap(Space::LOOSE)
        .fill_width()
        .any()
    })
}

fn verification(settings: &SpaceSettings) -> Element {
    column_of(VerificationLevel::ALL.map(|level| {
        let (label, detail) = verification_label(level);
        choice_row(
            settings.verification == level,
            label,
            detail,
            move |admin| {
                admin.edit_settings(|settings| settings.verification = level);
            },
        )
    }))
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn content_filter(settings: &SpaceSettings) -> Element {
    column_of(ContentFilter::ALL.map(|filter| {
        choice_row(
            settings.content_filter == filter,
            filter_label(filter),
            Label::from(""),
            move |admin| admin.edit_settings(|settings| settings.content_filter = filter),
        )
    }))
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn pause_row(label: Label, until: Option<i64>, directs: bool) -> Element {
    let now = now_ms();
    let paused = until.filter(|until| *until > now);
    row! {
        column! {
            text(label).label().semibold();
            if let Some(until) = paused {
                text(tr!("discord.admin.paused-until", time = sent_at(until).as_str())).caption().muted();
            }
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        match paused {
            Some(_) => button(text(tr!("discord.admin.resume")))
                .secondary()
                .small()
                .on_press(move |chat: &mut Chat| chat.pause_incident(directs, None))
                .any(),
            None => combo(None::<usize>)
                .placeholder(tr!("discord.admin.pause"))
                .options(PAUSES.map(|hours| tr!("discord.admin.pause-hours", count = hours)))
                .on_select(move |chat: &mut Chat, at: usize| {
                    let hours = PAUSES[at.min(PAUSES.len() - 1)];
                    chat.pause_incident(directs, Some(now_ms() + hours * HOUR_MS));
                })
                .any(),
        };
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn incidents(settings: &SpaceSettings) -> Element {
    column! {
        hint(tr!("discord.admin.incidents-hint"));
        pause_row(tr!("discord.admin.pause-invites"), settings.incidents.invites_paused_until, false);
        pause_row(tr!("discord.admin.pause-directs"), settings.incidents.directs_paused_until, true);
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

pub(super) fn delete(linked: &LinkedAccount, space: &SpaceId) -> Element {
    let (key, space) = (linked.key().to_owned(), *space);
    column! {
        hint(tr!("discord.manage.delete-space-body"));
        button(text(tr!("discord.manage.delete-space")))
            .danger()
            .on_press(move |chat: &mut Chat| chat.ask_delete_space(&key, &space));
    }
    .gap(Space::NORMAL)
    .fill_width()
    .any()
}

impl Chat {
    fn pause_incident(&mut self, directs: bool, until: Option<i64>) {
        let Some((key, space)) = self.admin_target() else {
            return;
        };
        let Some(current) = self
            .space_admin()
            .settings_original()
            .map(|settings| settings.incidents)
        else {
            return;
        };
        let (invites, dms) = match directs {
            true => (current.invites_paused_until, until),
            false => (until, current.directs_paused_until),
        };
        let now = now_ms();
        let invites = invites.filter(|until| *until > now);
        let dms = dms.filter(|until| *until > now);
        self.act(
            &key,
            move |client| async move { client.pause_incidents(&space, invites, dms).await },
            move |chat: &mut Chat, incidents: discord_core::Incidents| {
                if let Some(admin) = chat.admin_for(space) {
                    admin.incidents_changed(incidents);
                }
            },
        );
    }
}

impl SpaceAdmin {
    fn incidents_changed(&mut self, incidents: discord_core::Incidents) {
        if let Some(Ok(mut edited)) = (*self.settings).clone() {
            edited.original.incidents = incidents;
            edited.draft.incidents = incidents;
            self.settings.set(Some(Ok(edited)));
        }
    }
}
