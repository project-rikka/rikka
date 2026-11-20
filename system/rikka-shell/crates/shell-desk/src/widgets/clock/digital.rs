use desk_layout::Size;
use rikka_ui::{Element, Space, Styled, TextStyled, text};
use rikka_widgets::{column, spacer};

use crate::widgets::card::padded;
use crate::widgets::clock::Reading;

const SMALL: Digits = Digits::new(46.0, 38.0, 16.0);
const MEDIUM: Digits = Digits::new(64.0, 56.0, 24.0);
const LARGE: Digits = Digits::new(104.0, 88.0, 34.0);
const HERO: Digits = Digits::new(176.0, 148.0, 52.0);
const HERO_DATE: f32 = 24.0;

#[derive(Clone, Copy)]
struct Digits {
    alone: f32,
    with_seconds: f32,
    seconds: f32,
}

impl Digits {
    const fn new(alone: f32, with_seconds: f32, seconds: f32) -> Digits {
        Digits {
            alone,
            with_seconds,
            seconds,
        }
    }

    fn drawn(self, reading: &Reading) -> Element {
        match reading.seconds {
            true => reading.time(self.with_seconds, self.seconds),
            false => reading.time(self.alone, 0.0),
        }
    }
}

#[must_use]
pub(crate) fn view(reading: &Reading, size: Size) -> Element {
    match size {
        Size::Small => small(reading),
        Size::Medium => medium(reading),
        Size::Large => large(reading, LARGE, None),
        Size::ExtraLarge => large(reading, HERO, Some(HERO_DATE)),
    }
}

fn small(reading: &Reading) -> Element {
    padded(
        column! {
            reading.media_header().trailing(reading.week());
            spacer().grow(1.0);
            SMALL.drawn(reading);
            reading.date_line();
            spacer().grow(1.0);
            reading.day_ruler(false);
        }
        .fill(),
    )
}

fn medium(reading: &Reading) -> Element {
    let media_header = reading.kicker_or(reading.week_and_day());
    padded(
        column! {
            media_header;
            spacer().grow(1.0);
            MEDIUM.drawn(reading);
            text(reading.long_date()).label().muted().no_wrap();
            spacer().grow(1.0);
            reading.day_ruler(false);
        }
        .gap(Space::HAIR)
        .fill(),
    )
}

fn large(reading: &Reading, digits: Digits, date: Option<f32>) -> Element {
    let media_header = reading.kicker_or(reading.week_and_day());
    let dated = text(reading.long_date()).subheading().muted().no_wrap();
    let dated = match date {
        Some(size) => dated.text_size(size),
        None => dated,
    };
    padded(
        column! {
            media_header;
            spacer().grow(1.0);
            digits.drawn(reading);
            dated;
            spacer().grow(1.0);
            reading.day_ruler(true);
        }
        .gap(Space::TIGHT)
        .fill(),
    )
}
