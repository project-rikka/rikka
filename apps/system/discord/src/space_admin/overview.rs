use discord_core::{
    AFK_TIMEOUTS, BANNER, ChannelId, ChannelKind, INVITE_SPLASH, ImageUpload, NotifyLevel, Picture,
    SpaceId, SpaceSettings, SystemFlags,
};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::{SpaceAdmin, hint, loading};
use crate::account::LinkedAccount;
use crate::chat::Chat;
use crate::manage::{pick_icon, pick_picture};
use crate::menus::level_label;
use crate::widgets::picture_tile;

const ICON: f32 = 96.0;
const LONGEST_NAME: usize = 100;
const BANNER_SIDE: u32 = 1920;
const PICTURE_LIMIT: u64 = 10 * 1024 * 1024;
const CHOICE: f32 = 260.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Artwork {
    Icon,
    Banner,
    Splash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Backdrop {
    Banner,
    Splash,
}

pub(super) fn channels(
    linked: &LinkedAccount,
    space: &SpaceId,
    voice: bool,
) -> Vec<(ChannelId, String)> {
    linked
        .sections(space)
        .iter()
        .flat_map(|section| section.rows.iter())
        .map(|row| row.channel)
        .filter(|channel| match voice {
            true => channel.kind.is_voice(),
            false => matches!(channel.kind, ChannelKind::Text | ChannelKind::Announcement),
        })
        .map(|channel| (channel.id, channel.name.clone()))
        .collect()
}

pub(super) fn channel_combo(
    current: Option<ChannelId>,
    choices: Vec<(ChannelId, String)>,
    none: Option<Label>,
    set: impl Fn(&mut SpaceAdmin, Option<ChannelId>) + 'static,
) -> Element {
    let offset = usize::from(none.is_some());
    let at = current
        .and_then(|current| choices.iter().position(|(id, _)| *id == current))
        .map(|at| at + offset)
        .or_else(|| none.as_ref().map(|_| 0));
    let mut options: Vec<Label> = none.into_iter().collect();
    options.extend(
        choices
            .iter()
            .map(|(_, name)| Label::from(format!("# {name}"))),
    );
    combo(at)
        .options(options)
        .on_select(move |admin: &mut SpaceAdmin, index: usize| {
            let chosen = index
                .checked_sub(offset)
                .and_then(|at| choices.get(at))
                .map(|(id, _)| *id);
            set(admin, chosen);
        })
        .width(CHOICE)
        .any()
}

fn timeout_label(seconds: u32) -> Label {
    match seconds {
        3600 => tr!("discord.admin.afk-hour"),
        seconds => tr!("discord.admin.afk-minutes", count = i64::from(seconds / 60)),
    }
}

pub(super) fn view(admin: &SpaceAdmin, linked: &LinkedAccount, space: &SpaceId) -> Element {
    loading(&admin.settings, |edited| {
        let settings = &edited.draft;
        column! {
            section(tr!("discord.admin.identity"), identity(admin, settings));
            section(tr!("discord.admin.inactive"), inactive(linked, space, settings));
            section(tr!("discord.admin.system-messages"), system(linked, space, settings));
            section(tr!("discord.admin.default-notifications"), defaults(settings));
            section(tr!("discord.admin.banner"), artwork(admin, settings, Backdrop::Banner));
            section(tr!("discord.admin.splash"), artwork(admin, settings, Backdrop::Splash));
            section(
                tr!("discord.admin.progress-bar"),
                field(
                    tr!("discord.admin.progress-bar-show"),
                    switch(settings.progress_bar).on_toggle(|admin: &mut SpaceAdmin, on: bool| {
                        admin.edit_settings(|settings| settings.progress_bar = on);
                    }),
                ),
            );
        }
        .gap(Space::LOOSE)
        .fill_width()
        .any()
    })
}

fn pending_or(pending: Option<&Option<ImageUpload>>, current: Option<&Picture>) -> Option<bool> {
    match pending {
        Some(Some(_)) => Some(true),
        Some(None) => None,
        None => current.map(|_| false),
    }
}

fn identity(admin: &SpaceAdmin, settings: &SpaceSettings) -> Element {
    let pending = admin.images.icon.as_ref();
    let shown = match pending {
        Some(_) => None,
        None => settings.icon.as_ref(),
    };
    let marker = pending_or(pending, settings.icon.as_ref());
    row! {
        column! {
            picture_tile(shown, &settings.name, ICON, ICON / 2.0);
            if marker == Some(true) {
                text(tr!("discord.admin.picture-pending")).caption().muted();
            }
            row! {
                button(text(tr!("discord.admin.upload")))
                    .secondary()
                    .small()
                    .on_press(|chat: &mut Chat| chat.pick_space_artwork(Artwork::Icon));
                if marker.is_some() {
                    button(text(tr!("discord.admin.remove")))
                        .ghost()
                        .small()
                        .on_press(|admin: &mut SpaceAdmin| admin.set_artwork(Artwork::Icon, None));
                }
            }
            .gap(Space::TIGHT);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center);
        column! {
            text(tr!("discord.admin.space-name")).caption().semibold().muted();
            text_field(&admin.docs.name)
                .max_len(LONGEST_NAME)
                .on_change(|admin: &mut SpaceAdmin| {
                    let name = admin.docs.name.text();
                    admin.edit_settings(|settings| settings.name = name);
                })
                .fill_width();
            hint(tr!("discord.admin.icon-hint"));
        }
        .gap(Space::TIGHT)
        .grow(1.0)
        .min_width(0.0);
    }
    .gap(Space::LOOSE)
    .fill_width()
    .any()
}

fn inactive(linked: &LinkedAccount, space: &SpaceId, settings: &SpaceSettings) -> Element {
    let at = AFK_TIMEOUTS
        .iter()
        .position(|seconds| *seconds == settings.afk_timeout)
        .unwrap_or(1);
    column! {
        field(
            tr!("discord.admin.inactive-channel"),
            channel_combo(
                settings.afk_channel,
                channels(linked, space, true),
                Some(tr!("discord.admin.no-inactive-channel")),
                |admin, chosen| admin.edit_settings(|settings| settings.afk_channel = chosen),
            ),
        );
        field(
            tr!("discord.admin.inactive-timeout"),
            combo(at)
                .options(AFK_TIMEOUTS.map(timeout_label))
                .on_select(|admin: &mut SpaceAdmin, at: usize| {
                    admin.edit_settings(|settings| {
                        settings.afk_timeout = AFK_TIMEOUTS[at.min(AFK_TIMEOUTS.len() - 1)];
                    });
                })
                .width(CHOICE),
        );
        hint(tr!("discord.admin.inactive-hint"));
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn system_toggle(settings: &SpaceSettings, label: Label, flag: SystemFlags) -> Element {
    let shown = !settings.system_flags.contains(flag);
    field(
        label,
        switch(shown).on_toggle(move |admin: &mut SpaceAdmin, on: bool| {
            admin.edit_settings(|settings| {
                settings.system_flags = settings.system_flags.with(flag, !on);
            });
        }),
    )
    .any()
}

fn system(linked: &LinkedAccount, space: &SpaceId, settings: &SpaceSettings) -> Element {
    column! {
        field(
            tr!("discord.admin.system-channel"),
            channel_combo(
                settings.system_channel,
                channels(linked, space, false),
                Some(tr!("discord.admin.no-system-channel")),
                |admin, chosen| admin.edit_settings(|settings| settings.system_channel = chosen),
            ),
        );
        system_toggle(settings, tr!("discord.admin.system-joins"), SystemFlags::SUPPRESS_JOIN_NOTIFICATIONS);
        system_toggle(
            settings,
            tr!("discord.admin.system-join-replies"),
            SystemFlags::SUPPRESS_JOIN_NOTIFICATION_REPLIES,
        );
        system_toggle(
            settings,
            tr!("discord.admin.system-boosts"),
            SystemFlags::SUPPRESS_PREMIUM_SUBSCRIPTIONS,
        );
        system_toggle(
            settings,
            tr!("discord.admin.system-tips"),
            SystemFlags::SUPPRESS_GUILD_REMINDER_NOTIFICATIONS,
        );
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn defaults(settings: &SpaceSettings) -> Element {
    let levels = [NotifyLevel::All, NotifyLevel::Mentions];
    let at = levels
        .iter()
        .position(|level| *level == settings.default_notifications)
        .unwrap_or(0);
    column! {
        levels
            .iter()
            .fold(segmented(at), |choice, level| choice.segment(level_label(*level)))
            .on_select(move |admin: &mut SpaceAdmin, at: usize| {
                let level = levels[at.min(levels.len() - 1)];
                admin.edit_settings(|settings| settings.default_notifications = level);
            })
            .width(CHOICE);
        hint(tr!("discord.admin.default-notifications-hint"));
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

fn artwork(admin: &SpaceAdmin, settings: &SpaceSettings, backdrop: Backdrop) -> Element {
    let (kind, feature, current, pending) = match backdrop {
        Backdrop::Banner => (
            Artwork::Banner,
            BANNER,
            settings.banner.as_ref(),
            admin.images.banner.as_ref(),
        ),
        Backdrop::Splash => (
            Artwork::Splash,
            INVITE_SPLASH,
            settings.splash.as_ref(),
            admin.images.splash.as_ref(),
        ),
    };
    if !settings.has(feature) {
        return hint(tr!("discord.admin.needs-boost"));
    }
    let marker = pending_or(pending, current);
    column! {
        match (pending, current) {
            (None, Some(picture)) => picture_tile(Some(picture), &settings.name, 240.0, 8.0),
            _ => spacer().any(),
        };
        if marker == Some(true) {
            text(tr!("discord.admin.picture-pending")).caption().muted();
        }
        row! {
            button(text(tr!("discord.admin.upload")))
                .secondary()
                .small()
                .on_press(move |chat: &mut Chat| chat.pick_space_artwork(kind));
            if marker.is_some() {
                button(text(tr!("discord.admin.remove")))
                    .ghost()
                    .small()
                    .on_press(move |admin: &mut SpaceAdmin| admin.set_artwork(kind, None));
            }
        }
        .gap(Space::TIGHT);
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}

impl SpaceAdmin {
    pub(super) fn set_artwork(&mut self, kind: Artwork, image: Option<ImageUpload>) {
        let mut images = (*self.images).clone();
        match kind {
            Artwork::Icon => images.icon = Some(image),
            Artwork::Banner => images.banner = Some(image),
            Artwork::Splash => images.splash = Some(image),
        }
        self.images.set(images);
        self.warned.set(false);
    }
}

impl Chat {
    pub(super) fn pick_space_artwork(&mut self, kind: Artwork) {
        let parent = window_identifier();
        spawn_blocking(
            move || match kind {
                Artwork::Icon => pick_icon(parent),
                Artwork::Banner | Artwork::Splash => {
                    pick_picture(parent, BANNER_SIDE, PICTURE_LIMIT)
                        .map(|picked| picked.map(|(_, image)| image))
                }
            },
            move |chat: &mut Chat, picked: Result<Option<ImageUpload>, String>| match picked {
                Ok(Some(image)) => chat.space_admin_mut().set_artwork(kind, Some(image)),
                Ok(None) => {}
                Err(error_message) => chat.manage_failed(error_message),
            },
        );
    }
}
