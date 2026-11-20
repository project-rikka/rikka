use music_core::Track;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::blocks::MediaLine;
use crate::menus::{self, Subject, TrackContext, TrackTarget};
use crate::music::Music;
use crate::player::Player;
use crate::widgets::bars;
use crate::wording::length;

const WIDTH: f32 = 360.0;
const ENTRY: f32 = 60.0;

pub(crate) fn view(music: &Music) -> impl View {
    container(queue(music.player()))
        .padding(Inset::edges(
            Space::NORMAL,
            Space::NONE,
            Space::NONE,
            Space::NONE,
        ))
        .width(WIDTH)
        .fill_height()
}

fn queue(player: &Player) -> Element {
    if player.now().is_none() {
        return status_page(MaterialSymbol::QUEUE_MUSIC, tr!("music.queue.empty"))
            .fill()
            .any();
    }
    column! {
        caption_of(tr!("music.queue.now"));
        component(|player: &Player| player, now_playing);
        caption_of(tr!("music.queue.next"));
        component(|player: &Player| player, upcoming).grow(1.0).fill_width();
    }
    .gap(Space::TIGHT)
    .padding(Inset::xy(Space::SNUG, Space::NONE))
    .fill()
    .any()
}

fn now_playing(player: &Player) -> Element {
    match player.now() {
        Some(now) => entry(now, None, Some(player.playing())),
        None => Element::empty(),
    }
}

fn upcoming(player: &Player) -> Element {
    let listed = player.upcoming();
    if listed.is_empty() {
        return text(tr!("music.queue.end"))
            .caption()
            .muted()
            .fill_width()
            .any();
    }
    lazy_column(listed.len(), move |slot| {
        entry(&listed[slot.index], Some(slot.index), None)
    })
    .uniform(ENTRY)
    .fill()
    .any()
}

fn caption_of(title: Label) -> impl View {
    container(text(title).caption().muted().semibold())
        .padding(Inset::edges(
            Space::SNUG,
            Space::SNUG,
            Space::HAIR,
            Space::SNUG,
        ))
        .fill_width()
}

fn entry(track: &Track, ahead: Option<usize>, playing: Option<bool>) -> Element {
    let body = container(
        MediaLine {
            artwork: &track.artwork,
            side: 44.0,
            title: track.name.clone(),
            subtitle: track.byline(),
            lit: playing.is_some(),
            trailing: Some(match playing {
                Some(playing) => bars(playing, 14.0).any(),
                None => text(length(track.duration)).caption().muted().any(),
            }),
        }
        .view(),
    )
    .padding(Inset::xy(Space::SNUG, Space::TIGHT))
    .radius(Corner::Item);
    let (body, context) = match ahead {
        Some(ahead) => (
            body.on_press(move |player: &mut Player| player.skip_to(ahead))
                .fill()
                .any(),
            TrackContext::Upcoming(ahead),
        ),
        None => (body.fill().any(), TrackContext::Playing),
    };
    let track_target = TrackTarget {
        track: track.clone(),
        context,
    };
    menus::target(Subject::Track(track_target), body)
        .height(ENTRY)
        .fill_width()
}
