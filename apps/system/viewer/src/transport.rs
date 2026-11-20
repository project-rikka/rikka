use std::time::Duration;

use rikka_app::prelude::*;
use rikka_format::format_duration;

use crate::screen::Screen;

const VOLUME_WIDTH: f32 = 88.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Playback {
    pub(crate) playing: bool,
    pub(crate) position: Duration,
    pub(crate) duration: Option<Duration>,
    pub(crate) volume: Option<f32>,
    pub(crate) dead: bool,
}

pub(crate) fn transport(playback: Playback) -> impl View {
    let glyph = if playback.playing {
        MaterialSymbol::PAUSE
    } else {
        MaterialSymbol::PLAY_ARROW
    };

    row! {
        button(icon(glyph).extent(Length::px(20.0)))
            .icon_only()
            .enabled(!playback.dead)
            .on_press(|screen: &mut Screen| screen.play_pause());
        readout(format_duration(playback.position));
        scrubber(playback);
        readout(
            playback
                .duration
                .map_or_else(|| String::from("--:--"), format_duration),
        );
        if let Some(volume) = playback.volume {
            icon(MaterialSymbol::VOLUME_UP).extent(Length::px(18.0));
            slider(volume)
                .range(0.0..=1.0)
                .on_change(|screen: &mut Screen, volume| screen.set_volume(volume))
                .width(VOLUME_WIDTH);
        }
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
}

fn scrubber(playback: Playback) -> Element {
    let Some(duration) = playback.duration.filter(|duration| !duration.is_zero()) else {
        return slider(0.0).enabled(false).grow(1.0).build();
    };
    let seconds = duration.as_secs_f32();
    slider(playback.position.as_secs_f32().min(seconds))
        .range(0.0..=seconds)
        .on_change(|screen: &mut Screen, seconds| screen.seek(seconds))
        .grow(1.0)
        .build()
}

fn readout(clock: String) -> impl View {
    text(clock).caption().muted().no_wrap()
}
