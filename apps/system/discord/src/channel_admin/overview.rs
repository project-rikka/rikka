use discord_core::{
    ARCHIVE_MINUTES, ChannelKind, ChannelSettings, MAX_USER_LIMIT, MIN_BITRATE, SLOWMODES,
    VideoQuality, max_bitrate,
};
use rikka_app::prelude::*;
use rikka_widgets::column;

use super::ChannelAdmin;
use crate::account::LinkedAccount;
use crate::space_admin::{hint, loading};

const LONGEST_NAME: usize = 100;
const LONGEST_TOPIC: usize = 1024;
const CHOICE: f32 = 260.0;
const REGIONS: [&str; 13] = [
    "brazil",
    "hongkong",
    "india",
    "japan",
    "rotterdam",
    "russia",
    "singapore",
    "southafrica",
    "sydney",
    "us-central",
    "us-east",
    "us-south",
    "us-west",
];

fn seconds_label(seconds: u32) -> Label {
    match seconds {
        0 => tr!("discord.channel-admin.off"),
        seconds if seconds < 60 => tr!("discord.channel-admin.seconds", count = i64::from(seconds)),
        seconds if seconds < 3600 => {
            tr!(
                "discord.channel-admin.minutes",
                count = i64::from(seconds / 60)
            )
        }
        seconds => tr!(
            "discord.channel-admin.hours",
            count = i64::from(seconds / 3600)
        ),
    }
}

fn archive_label(minutes: u32) -> Label {
    match minutes {
        60 => tr!("discord.channel-admin.hours", count = 1),
        1440 => tr!("discord.channel-admin.days", count = 1),
        4320 => tr!("discord.channel-admin.days", count = 3),
        _ => tr!("discord.channel-admin.week"),
    }
}

pub(super) fn view(admin: &ChannelAdmin, linked: &LinkedAccount) -> Element {
    let tier = admin
        .opened
        .as_ref()
        .and_then(|opened| linked.channel(&opened.channel))
        .and_then(|channel| channel.space)
        .and_then(|space| linked.space(&space))
        .map_or(0, |space| space.premium_tier);
    loading(&admin.settings, |edited| {
        let settings = &edited.draft;
        let texting = matches!(
            settings.kind,
            ChannelKind::Text | ChannelKind::Announcement | ChannelKind::Forum
        );
        let voice = settings.kind.is_voice();
        column! {
            text(tr!("discord.channel-admin.name")).caption().semibold().muted();
            text_field(&admin.name)
                .max_len(LONGEST_NAME)
                .on_change(|admin: &mut ChannelAdmin| {
                    let name = admin.name.text();
                    admin.edit(|settings| settings.name = name);
                })
                .fill_width();
            if texting {
                text(tr!("discord.channel-admin.topic")).caption().semibold().muted();
                container(
                    text_editor(&admin.topic)
                        .wrap(true)
                        .fits_text(60.0..=200.0)
                        .on_change(|admin: &mut ChannelAdmin| {
                            let topic: String = admin.topic.text().chars().take(LONGEST_TOPIC).collect();
                            admin.edit(|settings| settings.topic = topic);
                        }),
                )
                .padding(Inset::all(Space::SNUG))
                .border(1.0, palette().outline)
                .radius(Corner::Control)
                .fill_width();
                slowmode(settings);
                if settings.kind != ChannelKind::Announcement {
                    archive(settings);
                }
            }
            if voice {
                voice_settings(settings, tier);
            }
            if settings.kind != ChannelKind::Category {
                field(
                    tr!("discord.channel-admin.nsfw"),
                    switch(settings.nsfw).on_toggle(|admin: &mut ChannelAdmin, on: bool| {
                        admin.edit(|settings| settings.nsfw = on);
                    }),
                );
                hint(tr!("discord.channel-admin.nsfw-hint"));
            }
        }
        .gap(Space::SNUG)
        .fill_width()
        .any()
    })
}

fn slowmode(settings: &ChannelSettings) -> Element {
    let forum = settings.kind == ChannelKind::Forum;
    let current = match forum {
        true => settings.thread_slowmode,
        false => settings.slowmode,
    };
    let at = SLOWMODES
        .iter()
        .position(|known| *known == current)
        .unwrap_or(0);
    field(
        tr!("discord.channel-admin.slowmode"),
        combo(at)
            .options(SLOWMODES.map(seconds_label))
            .on_select(move |admin: &mut ChannelAdmin, at: usize| {
                let seconds = SLOWMODES[at.min(SLOWMODES.len() - 1)];
                admin.edit(|settings| match forum {
                    true => settings.thread_slowmode = seconds,
                    false => settings.slowmode = seconds,
                });
            })
            .width(CHOICE),
    )
    .any()
}

fn archive(settings: &ChannelSettings) -> Element {
    let at = ARCHIVE_MINUTES
        .iter()
        .position(|known| *known == settings.archive_minutes)
        .unwrap_or(2);
    field(
        tr!("discord.channel-admin.archive"),
        combo(at)
            .options(ARCHIVE_MINUTES.map(archive_label))
            .on_select(|admin: &mut ChannelAdmin, at: usize| {
                let minutes = ARCHIVE_MINUTES[at.min(ARCHIVE_MINUTES.len() - 1)];
                admin.edit(|settings| settings.archive_minutes = minutes);
            })
            .width(CHOICE),
    )
    .any()
}

fn voice_settings(settings: &ChannelSettings, tier: u8) -> Element {
    let most = max_bitrate(tier);
    let kbps = settings.bitrate.clamp(MIN_BITRATE, most) / 1000;
    let region_at = settings
        .region
        .as_deref()
        .and_then(|region| REGIONS.iter().position(|known| *known == region))
        .map_or(0, |at| at + 1);
    let mut regions: Vec<Label> = vec![tr!("discord.channel-admin.region-auto")];
    regions.extend(REGIONS.map(Label::from));
    let quality_at = usize::from(settings.video_quality == VideoQuality::Full);
    column! {
        field(
            tr!("discord.channel-admin.bitrate", kbps = i64::from(kbps)),
            slider(kbps as f32)
                .range((MIN_BITRATE / 1000) as f32..=(most / 1000) as f32)
                .step(1.0)
                .on_commit(|admin: &mut ChannelAdmin, kbps: f32| {
                    admin.edit(|settings| settings.bitrate = kbps.round() as u32 * 1000);
                })
                .width(CHOICE),
        );
        field(
            tr!("discord.channel-admin.user-limit", count = i64::from(settings.user_limit)),
            slider(settings.user_limit as f32)
                .range(0.0..=MAX_USER_LIMIT as f32)
                .step(1.0)
                .on_commit(|admin: &mut ChannelAdmin, limit: f32| {
                    admin.edit(|settings| settings.user_limit = limit.round() as u32);
                })
                .width(CHOICE),
        );
        field(
            tr!("discord.channel-admin.region"),
            combo(region_at)
                .options(regions)
                .on_select(|admin: &mut ChannelAdmin, at: usize| {
                    let region = at.checked_sub(1).and_then(|at| REGIONS.get(at)).map(|region| (*region).to_owned());
                    admin.edit(|settings| settings.region = region);
                })
                .width(CHOICE),
        );
        field(
            tr!("discord.channel-admin.video-quality"),
            segmented(quality_at)
                .segment(tr!("discord.channel-admin.video-auto"))
                .segment(tr!("discord.channel-admin.video-full"))
                .on_select(|admin: &mut ChannelAdmin, at: usize| {
                    let quality = match at {
                        0 => VideoQuality::Auto,
                        _ => VideoQuality::Full,
                    };
                    admin.edit(|settings| settings.video_quality = quality);
                }),
        );
    }
    .gap(Space::SNUG)
    .fill_width()
    .any()
}
