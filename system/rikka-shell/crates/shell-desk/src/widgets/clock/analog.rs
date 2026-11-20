use desk_layout::Size;
use rikka_ui::{
    Align, Element, Justify, Space, Styled, TextAlign, TextStyled, View, palette, text,
};
use rikka_widgets::{column, container, row, spacer};

use crate::widgets::card::padded;
use crate::widgets::clock::Reading;
use crate::widgets::clock::dial::dial;

const SMALL: f32 = 128.0;
const SMALL_AWAY: f32 = 106.0;
const MEDIUM: f32 = 128.0;
const MEDIUM_TIME: f32 = 36.0;
const LARGE: f32 = 248.0;
const HERO: f32 = 292.0;
const HERO_TIME: f32 = 88.0;

#[must_use]
pub(crate) fn view(reading: &Reading, size: Size) -> Element {
    match size {
        Size::Small => small(reading),
        Size::Medium => beside(reading, MEDIUM, MEDIUM_TIME),
        Size::Large => large(reading),
        Size::ExtraLarge => beside(reading, HERO, HERO_TIME),
    }
}

fn face(reading: &Reading, diameter: f32) -> Element {
    dial(
        reading.wall.clone(),
        reading.resolved.zone(),
        reading.seconds,
        diameter,
    )
    .build()
}

fn small(reading: &Reading) -> Element {
    match reading.resolved.city() {
        Some(city) => padded(
            column! {
                face(reading, SMALL_AWAY);
                text(city.to_owned())
                    .caption()
                    .semibold()
                    .color(palette().accent)
                    .align(TextAlign::Center)
                    .no_wrap()
                    .fill_width();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center)
            .justify(Justify::Center)
            .fill(),
        ),
        None => padded(container(face(reading, SMALL)).center().fill()),
    }
}

fn beside(reading: &Reading, diameter: f32, time: f32) -> Element {
    let big = diameter >= HERO;
    let media_header = reading.kicker_or(reading.week());
    let dated = text(reading.long_date()).muted().no_wrap();
    let dated = match big {
        true => dated.subheading(),
        false => dated.label(),
    };
    padded(
        row! {
            face(reading, diameter);
            column! {
                media_header;
                spacer().grow(1.0);
                reading.time(time, 0.0);
                dated;
                spacer().grow(1.0);
                reading.day_ruler(big);
            }
            .gap(Space::TIGHT)
            .grow(1.0)
            .min_width(0.0)
            .fill_height();
        }
        .gap(match big {
            true => Space::WIDE,
            false => Space::NORMAL,
        })
        .align_items(Align::Center)
        .fill(),
    )
}

fn large(reading: &Reading) -> Element {
    padded(
        column! {
            reading.media_header().trailing(reading.dateline());
            container(face(reading, LARGE)).center().grow(1.0).fill_width();
        }
        .gap(Space::TIGHT)
        .fill(),
    )
}
