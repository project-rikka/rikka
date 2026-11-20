use discord_core::Privacy;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::Phase;
use crate::chat::Chat;
use crate::screen::{self, Phase as Showing};
use crate::widgets::live_badge;

const MARK: &str = "discord-voice-privacy";

pub(crate) fn view(chat: &Chat) -> Option<Element> {
    let calls = chat.calls();
    let seat = calls.seat()?;
    let linked = chat.linked(&seat.key);
    let channel = linked.and_then(|linked| linked.channel(&seat.channel));
    let place = match (linked, channel) {
        (Some(linked), Some(channel)) => {
            let title = linked.channel_title(channel);
            let space = channel
                .space
                .as_ref()
                .and_then(|space| linked.space(space))
                .map(|space| space.name.clone());
            let account = (chat.current_key() != Some(seat.key.as_str()))
                .then(|| linked.account().name.clone());
            [Some(title), space, account]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" / ")
        }
        _ => seat.channel.to_string(),
    };
    let (caption, tint) = match calls.phase() {
        Phase::Live => (tr!("discord.voice.connected"), palette().success),
        Phase::Connecting => (tr!("discord.voice.connecting"), palette().warning),
        Phase::Joining => (tr!("discord.voice.joining"), palette().warning),
    };
    let latency = calls
        .latency()
        .map(|latency| tr!("discord.voice.latency", ms = latency.as_millis() as i64));
    let code = match calls.privacy() {
        Some(Privacy::EndToEnd { code }) => Some(code.clone()),
        _ => None,
    };
    let connected = row! {
        column! {
            row! {
                icon(MaterialSymbol::SIGNAL_CELLULAR_ALT).extent(Length::px(16.0)).color(tint);
                text(caption).label().semibold().no_wrap().color(tint);
                if let Some(latency) = latency {
                    text(latency).caption().muted().no_wrap();
                }
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center);
            row! {
                if let Some(code) = code {
                    lock(code);
                }
                text(place).caption().muted().no_wrap().grow(1.0).min_width(0.0);
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .fill_width();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
        button(icon(MaterialSymbol::CALL_END).extent(Length::px(20.0)).color(palette().danger))
            .ghost()
            .icon_only()
            .small()
            .focusless()
            .on_press(|chat: &mut Chat| chat.leave_voice());
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width();
    Some(
        container(
            column! {
                connected;
                if calls.phase() == Phase::Live {
                    screen_row(chat);
                    streams_row(chat);
                }
            }
            .gap(Space::SNUG)
            .fill_width(),
        )
        .padding(Inset::xy(Space::NORMAL, Space::SNUG))
        .background(palette().background.mix(palette().surface, 0.5))
        .fill_width()
        .any(),
    )
}

fn screen_row(chat: &Chat) -> Element {
    let screens = chat.screens();
    let sharing = screens.sharing();
    let live = sharing.is_some_and(|sharing| sharing.phase == Showing::Live);
    let paused = sharing.is_some_and(|sharing| sharing.paused);
    let showable = (live || screens.viewing().is_some()) && !screens.stage_shown();
    let share = match sharing {
        Some(_) => wide(
            MaterialSymbol::STOP_SCREEN_SHARE,
            tr!("discord.screen.stop"),
            palette().danger,
        )
        .on_press(|chat: &mut Chat| chat.stop_sharing()),
        None => wide(
            MaterialSymbol::SCREEN_SHARE,
            tr!("discord.screen.screen"),
            palette().text,
        )
        .on_press(|chat: &mut Chat| chat.open_go_live()),
    };
    row! {
        share.build().keyed(screen::golive::MARK);
        if live {
            button(
                icon(match paused {
                    true => MaterialSymbol::PLAY_ARROW,
                    false => MaterialSymbol::PAUSE,
                })
                .extent(Length::px(18.0)),
            )
            .ghost()
            .icon_only()
            .small()
            .focusless()
            .on_press(move |chat: &mut Chat| chat.pause_sharing(!paused));
        }
        if showable {
            button(icon(MaterialSymbol::TV).extent(Length::px(18.0)))
                .ghost()
                .icon_only()
                .small()
                .focusless()
                .on_press(|chat: &mut Chat| chat.show_stage(true));
        }
        if let Some(sharing) = sharing {
            spacer().grow(1.0);
            match sharing.phase {
                Showing::Live => row! {
                    live_badge();
                    text(sharing.viewers.len().to_string()).caption().muted().no_wrap();
                }
                .gap(Space::TIGHT)
                .align_items(Align::Center)
                .any(),
                Showing::Choosing | Showing::Starting => spinner().any(),
            };
        }
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .fill_width()
    .any()
}

fn streams_row(chat: &Chat) -> Option<Element> {
    let seat = chat.calls().seat()?;
    let linked = chat.linked(&seat.key)?;
    let watched = chat.screens().viewing().map(|viewing| viewing.key);
    let shown: Vec<Element> = linked
        .voice_in(&seat.channel)
        .into_iter()
        .filter(|state| !linked.is_me(&state.user))
        .filter_map(|state| {
            let key = state.stream()?;
            let name = linked
                .name_of(state.space.as_ref(), &state.user)
                .unwrap_or_else(|| state.user.to_string());
            let account = seat.key.clone();
            let tint = match watched == Some(key) {
                true => palette().accent,
                false => palette().text,
            };
            Some(
                wide(MaterialSymbol::TV, name.into(), tint)
                    .on_press(move |chat: &mut Chat| chat.watch_stream(&account, key))
                    .any(),
            )
        })
        .collect();
    (!shown.is_empty()).then(|| {
        row_of(shown)
            .gap(Space::TIGHT)
            .wrap_items()
            .fill_width()
            .any()
    })
}

fn wide(symbol: MaterialSymbol, label: Label, tint: Color) -> Button {
    button(
        row! {
            icon(symbol).extent(Length::px(18.0)).color(tint);
            text(label).label().no_wrap().color(tint);
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .ghost()
    .small()
    .focusless()
}

fn lock(code: Option<String>) -> Element {
    local_state(false, move |hovered: bool, set: Setter<bool>| {
        let mut layers = vec![
            container(
                icon(MaterialSymbol::ENHANCED_ENCRYPTION)
                    .extent(Length::px(14.0))
                    .color(palette().success),
            )
            .on_hover(move |inside: bool| {
                set(inside);
                skip_view();
            })
            .build()
            .keyed(MARK),
        ];
        if hovered {
            let caption = match &code {
                Some(code) => tr!("discord.voice.privacy-code", code = code.as_str()),
                None => tr!("discord.voice.end-to-end"),
            };
            layers.push(
                container(text(caption).caption().no_wrap())
                    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
                    .background(palette().surface)
                    .border(1.0, palette().outline)
                    .radius(Corner::Control)
                    .build()
                    .anchored(Anchor::above(MARK).offset(0.0, -4.0)),
            );
        }
        row_of(layers)
    })
}
