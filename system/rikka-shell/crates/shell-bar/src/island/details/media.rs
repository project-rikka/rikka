use std::time::Duration;

use rikka_mpris::{Control, Player, Track};
use rikka_theme::Corner;
use rikka_ui::{
    Align, Justify, Length, MaterialSymbol, Space, Styled, TextStyled, View, palette, text,
};
use rikka_widgets::{ControlSize, button, column, icon, row, slider};
use shell_feeds::{cover, waveform};

use crate::island::clock::span;
use crate::island::cover_transition::{CoverFrame, CoverTarget};
use crate::island::live;
use crate::island::state::Island;

const ART: f32 = 56.0;
const COVER: CoverFrame = CoverFrame::new(ART, Corner::Control.radius());
const WAVE_BARS: usize = 7;
const WAVE_HEIGHT: f32 = 20.0;
const SYMBOL: f32 = 22.0;
const PLAY: f32 = 30.0;
const LINE: f32 = 2.0;

#[must_use]
pub(crate) fn cover_target(player: &Player) -> CoverTarget {
    CoverTarget::new(COVER, cover(player.track.as_ref()))
}

pub(crate) fn card(player: &Player, island: &Island) -> impl View {
    let track = player.track.as_ref();
    let spectrum = player.is_playing().then(|| island.spectrum()).flatten();
    column! {
        row! {
            COVER.seat().align_self(Align::Start);
            column! {
                text(live::title(player)).bold().no_wrap();
                if let Some(artist) = track.and_then(Track::artist) {
                    text(artist).muted().no_wrap();
                }
                text(player.identity.as_str()).caption().faint().no_wrap();
            }
            .gap(LINE)
            .grow(1.0);
            waveform(spectrum, WAVE_BARS, WAVE_HEIGHT).color(palette().accent)
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .min_height(Length::px(ART))
        .fill_width();
        if let Some(length) = track.and_then(|track| track.length).filter(|length| !length.is_zero()) {
            timeline(player, length, island);
        }
        controls(player)
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn timeline(player: &Player, length: Duration, island: &Island) -> impl View {
    let at = island.position(player).min(length);
    let fraction = at.as_secs_f32() / length.as_secs_f32();
    row! {
        text(span(at)).caption().muted().no_wrap();
        slider(fraction)
            .control_size(ControlSize::Small)
            .enabled(player.abilities.seek)
            .on_commit(|island: &mut Island, at: f32| island.seek(at))
            .grow(1.0);
        text(format!("-{}", span(length.saturating_sub(at)))).caption().muted().no_wrap();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn controls(player: &Player) -> impl View {
    let abilities = player.abilities;
    let (pausing, symbol) = match player.is_playing() {
        true => (Control::Pause, MaterialSymbol::PAUSE),
        false => (Control::Play, MaterialSymbol::PLAY_ARROW),
    };
    row! {
        press(MaterialSymbol::SKIP_PREVIOUS, SYMBOL, abilities.previous, Control::Previous);
        press(symbol, PLAY, abilities.play || abilities.pause, pausing);
        press(MaterialSymbol::SKIP_NEXT, SYMBOL, abilities.next, Control::Next);
    }
    .gap(Space::LOOSE)
    .justify(Justify::Center)
    .align_items(Align::Center)
    .fill_width()
}

fn press(symbol: MaterialSymbol, size: f32, enabled: bool, control: Control) -> impl View {
    button(icon(symbol).extent(Length::px(size)).filled())
        .icon_only()
        .ghost()
        .control_size(ControlSize::Medium)
        .enabled(enabled)
        .on_press(move |island: &mut Island| island.control(control.clone()))
}
