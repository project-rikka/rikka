use music_core::Repeat;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::artwork::cover;
use crate::blocks::heart;
use crate::marks::mark;
use crate::menus::{self, Subject, TrackContext, TrackTarget};
use crate::music::Music;
use crate::nav::Place;
use crate::player::Player;
use crate::signal;
use crate::widgets::scrubber;
use crate::wording::service;

pub(crate) const BAR: f32 = 88.0;
const CENTER_FLOOR: f32 = 360.0;
const CENTER: f32 = 680.0;
const DISC: f32 = 40.0;

pub(crate) fn view(music: &Music) -> impl View {
    let player = music.player();
    row! {
        container(playing(music))
            .align_y(Align::Center)
            .grow(1.0)
            .min_width(0.0)
            .fill_height();
        column! {
            transport(player);
            scrubber(player.clock())
                .on_seek(|player: &mut Player, fraction: f32| player.seek_fraction(fraction));
        }
        .gap(Space::HAIR)
        .align_items(Align::Center)
        .width(CENTER)
        .shrink(1.0)
        .min_width(CENTER_FLOOR);
        container(extras(music))
            .align_x(Align::End)
            .align_y(Align::Center)
            .grow(1.0)
            .min_width(0.0)
            .fill_height();
    }
    .gap(Space::ROOMY)
    .align_items(Align::Center)
    .padding(Inset::xy(Space::ROOMY, Space::SNUG))
    .height(BAR)
    .fill_width()
}

fn playing(music: &Music) -> Element {
    let Some(track) = music.player().now().cloned() else {
        return Element::empty();
    };
    let browsed = music.browses(&track);
    let saved = browsed && music.library().is_saved(&track.id);
    let album = track.album.id.clone().filter(|_| browsed);
    let artist = track
        .artists
        .first()
        .and_then(|artist| artist.id.clone())
        .filter(|_| browsed);
    let via = music.player().via().map(|via| {
        row! {
            icon(mark(via.mark)).extent(Length::px(13.0)).color(palette().text_muted);
            text(tr!("music.player.via", name = service(&via).as_str())).caption().muted().no_wrap();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center)
    });
    let track_target = TrackTarget {
        track: track.clone(),
        context: TrackContext::Playing,
    };
    let sounding = signal::badge(music, signal::PLAYER);
    let body = row! {
        container(cover(&track.artwork, 56.0, 8.0))
            .radius(Corner::Item)
            .shrink(0.0);
        column! {
            container(text(track.name.clone()).label().semibold().no_wrap())
                .on_press(move |music: &mut Music| {
                    if let Some(album) = album.clone() {
                        music.go(Place::Album(album));
                    }
                });
            container(text(track.byline()).caption().muted().no_wrap())
                .on_press(move |music: &mut Music| {
                    if let Some(artist) = artist.clone() {
                        music.go(Place::Artist(artist));
                    }
                });
            sounding;
            via;
        }
        .gap(Space::HAIR)
        .shrink(1.0)
        .min_width(0.0);
        browsed.then(|| heart(track.clone(), saved));
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width();
    menus::target(Subject::Track(track_target), body)
        .fill_width()
        .transition(fade())
        .keyed(track.uri())
}

fn transport(player: &Player) -> impl View {
    let ready = player.now().is_some();
    let repeat = player.repeat();
    row! {
        toggle(MaterialSymbol::SHUFFLE, player.shuffle(), ready)
            .on_press(|player: &mut Player| player.toggle_shuffle());
        skip(MaterialSymbol::SKIP_PREVIOUS, ready)
            .on_press(|player: &mut Player| player.previous());
        disc(player.playing(), player.loading(), ready);
        skip(MaterialSymbol::SKIP_NEXT, ready)
            .on_press(|player: &mut Player| player.next());
        toggle(
            match repeat {
                Repeat::One => MaterialSymbol::REPEAT_ONE,
                Repeat::Off | Repeat::All => MaterialSymbol::REPEAT,
            },
            repeat != Repeat::Off,
            ready,
        )
        .on_press(|player: &mut Player| player.cycle_repeat());
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}

fn toggle(symbol: MaterialSymbol, on: bool, ready: bool) -> Button {
    button(icon(symbol).extent(Length::px(20.0)).color(match on {
        true => palette().accent,
        false => palette().text_muted,
    }))
    .ghost()
    .icon_only()
    .enabled(ready)
}

fn skip(symbol: MaterialSymbol, ready: bool) -> Button {
    let theme = theme();
    button(
        icon(symbol)
            .filled()
            .extent(Length::px(26.0))
            .color(theme.dim(theme.palette().text_muted, !ready)),
    )
    .ghost()
    .icon_only()
    .enabled(ready)
}

fn disc(playing: bool, loading: bool, ready: bool) -> impl View {
    let theme = theme();
    let palette = theme.palette();
    let symbol = match playing {
        true => MaterialSymbol::PAUSE,
        false => MaterialSymbol::PLAY_ARROW,
    };
    let face: Element = match loading && playing {
        true => spinner()
            .extent(Length::px(18.0))
            .color(palette.on_accent)
            .any(),
        false => icon(symbol)
            .filled()
            .extent(Length::px(26.0))
            .color(theme.dim(palette.on_accent, !ready))
            .any()
            .transition(scale(0.6))
            .keyed(playing),
    };
    container(face)
        .center()
        .background(theme.dim(palette.accent, !ready))
        .radius(DISC / 2.0)
        .on_press(|player: &mut Player| player.toggle())
        .size(Length::px(DISC), Length::px(DISC))
}

fn extras(music: &Music) -> Element {
    let volume = music.player().volume();
    row! {
        queue_toggle(music.queue_open())
            .on_press(|music: &mut Music| music.toggle_queue());
        button(
            icon(loudness(volume))
            .extent(Length::px(20.0))
            .color(palette().text_muted),
        )
        .ghost()
        .icon_only()
        .on_press(|player: &mut Player| player.toggle_mute());
        slider(volume)
            .control_size(ControlSize::Small)
            .on_change(|player: &mut Player, level: f32| player.set_volume(level))
            .width(Length::px(112.0));
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .any()
}

fn loudness(volume: f32) -> MaterialSymbol {
    if volume <= 0.0 {
        MaterialSymbol::VOLUME_OFF
    } else if volume < 0.5 {
        MaterialSymbol::VOLUME_DOWN
    } else {
        MaterialSymbol::VOLUME_UP
    }
}

fn queue_toggle(on: bool) -> Button {
    button(
        icon(MaterialSymbol::QUEUE_MUSIC)
            .extent(Length::px(20.0))
            .color(match on {
                true => palette().accent,
                false => palette().text_muted,
            }),
    )
    .ghost()
    .icon_only()
}
