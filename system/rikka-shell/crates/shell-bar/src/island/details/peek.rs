use rikka_mpris::Track;
use rikka_theme::Corner;
use rikka_ui::{Align, Element, Length, Space, Styled, TextStyled, View, palette, text};
use rikka_widgets::{column, icon, progress, row};
use shell_activity::ActivityEvent;
use shell_feeds::{cover, percent, waveform};

use crate::island::compact::tinted;
use crate::island::cover_transition::{CoverFrame, CoverTarget};
use crate::island::notice::Notice;
use crate::island::state::Island;

const SYMBOL: f32 = 24.0;
const ART: f32 = 40.0;
const COVER: CoverFrame = CoverFrame::new(ART, Corner::Item.radius());
const WAVE_BARS: usize = 7;
const WAVE_HEIGHT: f32 = 18.0;
const LINE: f32 = 4.0;

pub(crate) fn view(
    activity_event: &ActivityEvent,
    island: &Island,
) -> (Element, Option<CoverTarget>) {
    match (activity_event, Notice::from_event(activity_event)) {
        (ActivityEvent::Track { track, .. }, _) => (
            playing(track, island),
            Some(CoverTarget::new(COVER, cover(Some(track)))),
        ),
        (_, Some(notice)) => (notice_row(notice), None),
        (_, None) => (Element::empty(), None),
    }
}

fn playing(track: &Track, island: &Island) -> Element {
    row! {
        COVER.seat().align_self(Align::Start);
        column! {
            text(track.title.as_str()).bold().no_wrap();
            if let Some(artist) = track.artist() {
                text(artist).muted().no_wrap();
            }
        }
        .gap(LINE / 2.0)
        .grow(1.0);
        waveform(island.spectrum(), WAVE_BARS, WAVE_HEIGHT).color(palette().accent)
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .min_height(Length::px(ART))
    .fill_width()
    .build()
}

fn notice_row(notice: Notice) -> Element {
    let palette = palette();
    let mark = icon(notice.symbol)
        .extent(Length::px(SYMBOL))
        .filled()
        .color(tinted(notice.tone, palette));
    match notice.level {
        Some(level) => row! {
            mark;
            column! {
                text(notice.title).caption().muted().no_wrap();
                progress(level).color(palette.accent);
            }
            .gap(LINE)
            .grow(1.0);
            text(percent(level)).label().medium().no_wrap();
        },
        None => row! {
            mark;
            column! {
                text(notice.title).label().medium().no_wrap();
                if let Some(detail) = notice.detail {
                    text(detail).caption().muted().no_wrap();
                }
            }
            .gap(LINE / 2.0)
            .grow(1.0);
        },
    }
    .gap(Space::NORMAL)
    .align_items(Align::Center)
    .fill_width()
    .build()
}
