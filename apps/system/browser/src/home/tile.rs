use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::widgets::{GripView, grip};

pub(crate) const GLYPH: f32 = 24.0;
const WIDTH: f32 = 96.0;
const HEIGHT: f32 = 92.0;
const BADGE: f32 = 48.0;

pub(crate) fn tile(look: Element, label: String) -> GripView {
    grip(
        column! {
            container(look)
                .center()
                .radius(Corner::Panel)
                .background(palette().text.with_alpha(0.06))
                .size(Length::px(BADGE), Length::px(BADGE));
            text(label)
                .caption()
                .no_wrap()
                .align(TextAlign::Center)
                .min_width(0.0)
                .max_width(WIDTH - Space::SNUG.px() * 2.0);
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .padding(Inset::xy(Space::SNUG, Space::NORMAL))
        .fill(),
    )
    .size(Length::px(WIDTH), Length::px(HEIGHT))
}

pub(crate) fn shelf(heading: Label, tiles: Vec<Element>) -> Element {
    column! {
        text(heading).label().semibold().muted();
        row_of(tiles)
            .wrap_items()
            .gap(Space::TIGHT)
            .justify(Justify::Center)
            .fill_width();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .any()
}
