use std::ops::RangeInclusive;

use rikka_app::prelude::*;
use rikka_widgets::row;

const SLIDER: f32 = 220.0;
const READING: f32 = 64.0;

pub(crate) fn amount<M>(
    value: f32,
    range: RangeInclusive<f32>,
    step: f32,
    reading: String,
    change: impl IntoHandler<(f32,), (), M>,
) -> Flex {
    scale(value, range, step, reading, change, None)
}

pub(crate) fn dimmed_amount<M>(
    value: f32,
    range: RangeInclusive<f32>,
    step: f32,
    reading: String,
    change: impl IntoHandler<(f32,), (), M>,
    dimmed: bool,
) -> Flex {
    let faded = dimmed.then(|| theme().dim(palette().text, true));
    scale(value, range, step, reading, change, faded)
}

fn scale<M>(
    value: f32,
    range: RangeInclusive<f32>,
    step: f32,
    reading: String,
    change: impl IntoHandler<(f32,), (), M>,
    faded: Option<Color>,
) -> Flex {
    let readout = text(reading)
        .label()
        .no_wrap()
        .align(TextAlign::End)
        .width(READING);
    row! {
        slider(value)
            .range(range)
            .step(step)
            .on_change(change)
            .enabled(faded.is_none())
            .width(SLIDER);
        match faded {
            Some(ink) => readout.color(ink),
            None => readout,
        }
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
}
