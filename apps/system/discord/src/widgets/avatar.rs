use discord_core::{Picture, Status};
use rikka_app::prelude::*;

use crate::pictures;
use crate::widgets::status::status_color;

pub(crate) fn picture_tile(
    picture: Option<&Picture>,
    name: &str,
    side: f32,
    radius: f32,
) -> Element {
    match picture.and_then(|picture| pictures::fetched(picture, side)) {
        Some(bytes) => image(bytes)
            .fit(ImageFit::Cover)
            .radius(radius)
            .size(Length::px(side), Length::px(side))
            .any(),
        None => container(
            text(initials(name))
                .text_size(Length::px((side * 0.36).max(9.0)))
                .medium()
                .no_wrap(),
        )
        .center()
        .background(palette().surface_variant)
        .radius(radius)
        .size(Length::px(side), Length::px(side))
        .any(),
    }
}

pub(crate) fn avatar(picture: Option<&Picture>, name: &str, side: f32) -> Element {
    picture_tile(picture, name, side, side / 2.0)
}

pub(crate) fn avatar_with_status(
    picture: Option<&Picture>,
    name: &str,
    side: f32,
    status: Status,
    ring: Color,
) -> Element {
    let dot = (side * 0.4).round().max(8.0);
    let at = side - dot + 2.0;
    container(column_of([
        avatar(picture, name, side),
        container(spacer())
            .background(status_color(status))
            .border(2.0, ring)
            .radius(dot / 2.0)
            .size(Length::px(dot), Length::px(dot))
            .position(at, at)
            .any(),
    ]))
    .size(Length::px(side), Length::px(side))
    .any()
}

fn initials(name: &str) -> String {
    let words: String = name
        .split_whitespace()
        .filter_map(|word| word.chars().next())
        .take(3)
        .collect();
    match words.is_empty() {
        true => name.chars().take(1).collect(),
        false => words,
    }
}
