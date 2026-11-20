use std::collections::BTreeSet;

use discord_stream::{FrameRate, Resolution};
use rikka_app::prelude::*;
use rikka_audio::AudioApp;
use rikka_widgets::{column, row};

use crate::chat::Chat;
use crate::screen::{Setup, SoundPick};

pub(crate) const MARK: &str = "discord-screen-share";
const WIDTH: f32 = 340.0;
const APPS_HEIGHT: f32 = 160.0;

pub(crate) fn view(chat: &Chat) -> Option<Element> {
    let setup = chat.screens().setup()?.clone();
    let place = chat
        .calls()
        .seat()
        .and_then(|seat| {
            let linked = chat.linked(&seat.key)?;
            linked
                .channel(&seat.channel)
                .map(|channel| linked.channel_title(channel))
        })
        .unwrap_or_default();
    Some(
        popover(
            column! {
                column! {
                    text(tr!("discord.screen.share")).label().semibold().no_wrap();
                    text(tr!("discord.screen.streaming-to", place = place.as_str()))
                        .caption()
                        .muted()
                        .no_wrap();
                }
                .gap(Space::HAIR)
                .fill_width();
                choice(tr!("discord.screen.resolution"), resolutions(&setup));
                choice(tr!("discord.screen.frame-rate"), frame_rates(&setup));
                choice(tr!("discord.screen.sound"), sounds(&setup));
                if let SoundPick::Apps(chosen) = &setup.sound {
                    apps(setup.apps.as_deref(), chosen);
                }
                row! {
                    spacer().grow(1.0);
                    button(text(tr!("discord.screen.cancel")))
                        .ghost()
                        .on_press(|chat: &mut Chat| chat.close_go_live());
                    button(
                        row! {
                            icon(MaterialSymbol::SCREEN_SHARE).extent(Length::px(16.0));
                            text(tr!("discord.screen.go-live")).label();
                        }
                        .gap(Space::TIGHT)
                        .align_items(Align::Center),
                    )
                    .primary()
                    .on_press(|chat: &mut Chat| chat.go_live());
                }
                .gap(Space::SNUG)
                .align_items(Align::Center)
                .fill_width();
            }
            .gap(Space::NORMAL)
            .padding(Inset::all(Space::SNUG))
            .width(WIDTH),
        )
        .on_dismiss(|chat: &mut Chat| chat.close_go_live())
        .build()
        .anchored(Anchor::above(MARK).offset(0.0, -8.0)),
    )
}

fn choice(title: Label, control: impl View) -> impl View {
    column! {
        text(title).caption().semibold().muted().no_wrap();
        control;
    }
    .gap(Space::TIGHT)
    .fill_width()
}

fn resolutions(setup: &Setup) -> impl View {
    let chosen = Resolution::ALL
        .iter()
        .position(|resolution| *resolution == setup.quality.resolution)
        .unwrap_or_default();
    segmented(chosen)
        .segments(Resolution::ALL.map(resolution_label))
        .on_select(|chat: &mut Chat, at: usize| {
            if let Some(resolution) = Resolution::ALL.get(at) {
                chat.choose_resolution(*resolution);
            }
        })
        .focusless()
        .fill_width()
}

fn resolution_label(resolution: Resolution) -> Label {
    match resolution.height() {
        Some(2160) => "4K".into(),
        Some(height) => format!("{height}p").into(),
        None => tr!("discord.screen.source"),
    }
}

fn frame_rates(setup: &Setup) -> impl View {
    let chosen = FrameRate::ALL
        .iter()
        .position(|rate| *rate == setup.quality.frame_rate)
        .unwrap_or_default();
    segmented(chosen)
        .segments(
            FrameRate::ALL
                .map(|rate| tr!("discord.screen.fps", fps = i64::from(rate.per_second()))),
        )
        .on_select(|chat: &mut Chat, at: usize| {
            if let Some(rate) = FrameRate::ALL.get(at) {
                chat.choose_frame_rate(*rate);
            }
        })
        .focusless()
        .fill_width()
}

fn sounds(setup: &Setup) -> impl View {
    let chosen = match setup.sound {
        SoundPick::Silent => 0,
        SoundPick::Everything => 1,
        SoundPick::Apps(_) => 2,
    };
    segmented(chosen)
        .segment(tr!("discord.screen.no-sound"))
        .segment(tr!("discord.screen.everything"))
        .segment(tr!("discord.screen.apps"))
        .on_select(|chat: &mut Chat, at: usize| {
            chat.choose_sound(match at {
                1 => SoundPick::Everything,
                2 => SoundPick::Apps(BTreeSet::new()),
                _ => SoundPick::Silent,
            });
        })
        .focusless()
        .fill_width()
}

fn apps(clients: Option<&[AudioApp]>, chosen: &BTreeSet<String>) -> Element {
    let Some(clients) = clients else {
        return row! {
            spinner();
            text(tr!("discord.screen.finding-apps")).caption().muted();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .any();
    };
    if clients.is_empty() {
        return text(tr!("discord.screen.no-apps")).caption().muted().any();
    }
    scrollable(
        column_of(clients.iter().map(|app| {
            let key = app.key.clone();
            checkbox(chosen.contains(&app.key))
                .label(text(app.name.clone()).label().no_wrap())
                .on_toggle(move |chat: &mut Chat, on: bool| chat.toggle_shared_app(&key, on))
                .any()
        }))
        .gap(Space::TIGHT)
        .fill_width(),
    )
    .max_height(APPS_HEIGHT)
    .fill_width()
    .any()
}
